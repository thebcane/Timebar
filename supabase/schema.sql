-- TimeBar sync schema — paste into Supabase → SQL Editor → Run.
-- Mirrors the local SQLite tables. Timestamps are epoch milliseconds (int8).
-- Row-Level Security scopes every row to the signed-in user.

create table if not exists public.folders (
  id         text primary key,
  name       text not null,
  color      text not null,
  parent_id  text,
  sort_order int8 not null default 0,
  archived   boolean not null default false,
  created_at int8 not null,
  updated_at int8 not null,
  deleted_at int8,
  user_id    uuid not null default auth.uid()
);

create table if not exists public.tasks (
  id          text primary key,
  name        text not null,
  color       text not null,
  folder_id   text,
  hourly_rate float8 not null default 0,
  currency    text not null default 'USD',
  billable    boolean not null default false,
  sort_order  int8 not null default 0,
  archived    boolean not null default false,
  created_at  int8 not null,
  updated_at  int8 not null,
  deleted_at  int8,
  user_id     uuid not null default auth.uid()
);

create table if not exists public.sessions (
  id            text primary key,
  task_id       text not null,
  started_at    int8 not null,
  ended_at      int8,
  note          text,
  rate_snapshot float8 not null default 0,
  billed        boolean not null default false,
  source        text not null default 'timer',
  created_at    int8 not null,
  updated_at    int8 not null,
  deleted_at    int8,
  user_id       uuid not null default auth.uid()
);

-- Fast "what changed since X" pulls.
create index if not exists folders_updated_idx  on public.folders(user_id, updated_at);
create index if not exists tasks_updated_idx    on public.tasks(user_id, updated_at);
create index if not exists sessions_updated_idx on public.sessions(user_id, updated_at);

-- Row-Level Security: a user can only see/modify their own rows.
alter table public.folders  enable row level security;
alter table public.tasks    enable row level security;
alter table public.sessions enable row level security;

create policy "own folders"  on public.folders  for all
  using (user_id = auth.uid()) with check (user_id = auth.uid());
create policy "own tasks"    on public.tasks    for all
  using (user_id = auth.uid()) with check (user_id = auth.uid());
create policy "own sessions" on public.sessions for all
  using (user_id = auth.uid()) with check (user_id = auth.uid());
