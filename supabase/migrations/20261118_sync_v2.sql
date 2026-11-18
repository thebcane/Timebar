-- TimeBar sync v2 — paste into Supabase → SQL Editor → Run, ONCE, on a project
-- that already has schema.sql applied. Safe to re-run.
--
-- Run this BEFORE installing an app build that includes sync v2: the new
-- client pulls by `server_seq`, which doesn't exist until this has run.
--
-- What it adds:
--   1. server_seq — a server-assigned change counter. Clients pull
--      "everything with server_seq > my cursor" instead of trusting their own
--      clocks, so a device with a slow clock can no longer lose edits.
--   2. Last-write-wins on the server. An incoming row older than what's stored
--      (by updated_at) is ignored instead of overwriting newer data.
--   3. One running timer per user. When a running session arrives, any other
--      running session is closed at the moment the newer one started.
--   4. *_recent views (rows changed in the last 2 minutes). server_seq values
--      are handed out before commit, so when two devices push at the same
--      moment a lower number can become visible after a higher one. Clients
--      re-check these views each cycle so such a row is never skipped.

-- 1. Change counter ------------------------------------------------------------

create sequence if not exists public.sync_seq;
grant usage, select on sequence public.sync_seq to authenticated, service_role;

alter table public.folders  add column if not exists server_seq int8;
alter table public.tasks    add column if not exists server_seq int8;
alter table public.sessions add column if not exists server_seq int8;

alter table public.folders  add column if not exists server_at timestamptz not null default now();
alter table public.tasks    add column if not exists server_at timestamptz not null default now();
alter table public.sessions add column if not exists server_at timestamptz not null default now();

-- Backfill existing rows in updated_at order so the first v2 pull sees them.
update public.folders  f set server_seq = s.seq from (
  select id, nextval('public.sync_seq') as seq from public.folders  where server_seq is null order by updated_at
) s where f.id = s.id;
update public.tasks    t set server_seq = s.seq from (
  select id, nextval('public.sync_seq') as seq from public.tasks    where server_seq is null order by updated_at
) s where t.id = s.id;
update public.sessions x set server_seq = s.seq from (
  select id, nextval('public.sync_seq') as seq from public.sessions where server_seq is null order by updated_at
) s where x.id = s.id;

alter table public.folders  alter column server_seq set not null;
alter table public.tasks    alter column server_seq set not null;
alter table public.sessions alter column server_seq set not null;

create index if not exists folders_seq_idx  on public.folders(user_id, server_seq);
create index if not exists tasks_seq_idx    on public.tasks(user_id, server_seq);
create index if not exists sessions_seq_idx on public.sessions(user_id, server_seq);
create index if not exists folders_at_idx   on public.folders(user_id, server_at);
create index if not exists tasks_at_idx     on public.tasks(user_id, server_at);
create index if not exists sessions_at_idx  on public.sessions(user_id, server_at);

-- 2. Stamp server_seq on every write + last-write-wins guard --------------------

create or replace function public.timebar_before_write()
returns trigger
language plpgsql
set search_path = public
as $$
begin
  -- An older version of the row must not overwrite a newer one. Returning
  -- NULL from a BEFORE trigger skips the write for this row only, so a stale
  -- push is silently dropped (the client will pull the newer row next cycle).
  if tg_op = 'UPDATE' and new.updated_at < old.updated_at then
    return null;
  end if;
  -- Server-owned columns: never trust the client for these.
  new.server_seq := nextval('public.sync_seq');
  new.server_at  := clock_timestamp();
  if tg_op = 'UPDATE' then
    new.user_id := old.user_id;
  end if;
  return new;
end;
$$;

drop trigger if exists folders_before_write  on public.folders;
drop trigger if exists tasks_before_write    on public.tasks;
drop trigger if exists sessions_before_write on public.sessions;

create trigger folders_before_write  before insert or update on public.folders
  for each row execute function public.timebar_before_write();
create trigger tasks_before_write    before insert or update on public.tasks
  for each row execute function public.timebar_before_write();
create trigger sessions_before_write before insert or update on public.sessions
  for each row execute function public.timebar_before_write();

-- 3. One running timer per user ------------------------------------------------
--
-- Rule (mirrored in the app, see reconcile_running in sync.rs): the running
-- session that started LAST wins. Every other running session ends when the
-- winner started (or 1 ms after its own start, so it never has zero length).

create or replace function public.timebar_single_running()
returns trigger
language plpgsql
set search_path = public
as $$
declare
  now_ms int8 := (extract(epoch from clock_timestamp()) * 1000)::int8;
  newer_start int8;
begin
  -- Is there a running session that started after this one? Then this one loses.
  select min(started_at) into newer_start
  from public.sessions
  where user_id = new.user_id
    and id <> new.id
    and ended_at is null
    and deleted_at is null
    and started_at > new.started_at;

  if newer_start is not null then
    update public.sessions
       set ended_at   = greatest(newer_start, started_at + 1),
           updated_at = greatest(now_ms, updated_at + 1)
     where id = new.id;
  end if;

  -- Close every other running session that started at or before this one.
  -- (If this one just lost, the loop above already closed it; the remaining
  -- winner is left alone because it started later.)
  update public.sessions
     set ended_at   = greatest(new.started_at, started_at + 1),
         updated_at = greatest(now_ms, updated_at + 1)
   where user_id = new.user_id
     and id <> new.id
     and ended_at is null
     and deleted_at is null
     and started_at <= new.started_at
     and newer_start is null;

  return null; -- AFTER trigger: return value ignored
end;
$$;

drop trigger if exists sessions_single_running on public.sessions;

-- Only fires for running, live sessions. The updates above set ended_at, so
-- they don't re-trigger this (no recursion).
create trigger sessions_single_running
  after insert or update on public.sessions
  for each row
  when (new.ended_at is null and new.deleted_at is null)
  execute function public.timebar_single_running();

-- 4. Recently changed rows -------------------------------------------------------
-- security_invoker: the views run with the caller's rights, so RLS still
-- limits each user to their own rows.

create or replace view public.folders_recent with (security_invoker = true) as
  select * from public.folders  where server_at > now() - interval '2 minutes';
create or replace view public.tasks_recent with (security_invoker = true) as
  select * from public.tasks    where server_at > now() - interval '2 minutes';
create or replace view public.sessions_recent with (security_invoker = true) as
  select * from public.sessions where server_at > now() - interval '2 minutes';

grant select on public.folders_recent, public.tasks_recent, public.sessions_recent to authenticated;

-- Clean up any doubles that already exist: keep each user's newest running
-- session, close the rest at the moment it started.
with ranked as (
  select id, user_id, started_at,
         first_value(started_at) over (partition by user_id order by started_at desc) as winner_start,
         row_number() over (partition by user_id order by started_at desc) as rn
  from public.sessions
  where ended_at is null and deleted_at is null
)
update public.sessions s
   set ended_at   = greatest(r.winner_start, s.started_at + 1),
       updated_at = greatest((extract(epoch from clock_timestamp()) * 1000)::int8, s.updated_at + 1)
  from ranked r
 where s.id = r.id and r.rn > 1;
