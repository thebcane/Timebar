import { useEffect, useState } from "react";
import { RefreshCw, LogOut } from "lucide-react";
import { formatDistanceToNow } from "date-fns";
import { Modal, Field, TextInput, Btn } from "./ui";
import * as ipc from "../lib/ipc";
import { useStore } from "../store/store";
import type { SyncStatus } from "../lib/types";

export function SyncPanel({ onClose }: { onClose: () => void }) {
  const bump = useStore((s) => s.bump);
  const refreshActive = useStore((s) => s.refreshActive);

  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const refresh = async () => setStatus(await ipc.syncStatus());
  useEffect(() => {
    refresh();
  }, []);

  const afterAuth = async () => {
    await ipc.syncNow().catch(() => 0);
    await refresh();
    bump();
    refreshActive();
  };

  const signIn = async () => {
    setBusy(true);
    setMsg(null);
    try {
      await ipc.syncSignIn(email, password);
      await afterAuth();
    } catch (e) {
      setMsg(String(e));
    }
    setBusy(false);
  };

  const signUp = async () => {
    setBusy(true);
    setMsg(null);
    try {
      const r = await ipc.syncSignUp(email, password);
      if (r === "confirm-email") {
        setMsg("Account created — check your email to confirm, then sign in.");
      } else {
        await afterAuth();
      }
    } catch (e) {
      setMsg(String(e));
    }
    setBusy(false);
  };

  const signOut = async () => {
    await ipc.syncSignOut();
    setEmail("");
    setPassword("");
    await refresh();
  };

  const changePassword = async () => {
    setBusy(true);
    setMsg(null);
    try {
      await ipc.syncChangePassword(newPassword);
      setNewPassword("");
      setMsg("Password updated.");
    } catch (e) {
      setMsg(String(e));
    }
    setBusy(false);
  };

  const syncNow = async () => {
    setBusy(true);
    setMsg(null);
    try {
      const n = await ipc.syncNow();
      setMsg(`Synced — ${n} update${n === 1 ? "" : "s"} pulled.`);
      await refresh();
      bump();
      refreshActive();
    } catch (e) {
      setMsg(String(e));
    }
    setBusy(false);
  };

  return (
    <Modal title="Sync across devices" onClose={onClose}>
      {status?.signedIn ? (
        <>
          <div className="mb-2 text-[13px] text-text-secondary">
            Signed in as <span className="font-medium text-text-primary">{status.email}</span>
          </div>
          <div className="mb-3 text-[12px] text-text-muted">
            Last sync:{" "}
            {status.lastSync
              ? `${formatDistanceToNow(new Date(status.lastSync))} ago`
              : "never"}
          </div>
          {status.lastError ? (
            <div className="mb-3 rounded-pill bg-panel-raised px-3 py-2 text-[12px] text-task-pink">
              {status.lastError}
            </div>
          ) : null}
          <div className="flex gap-2">
            <Btn variant="primary" onClick={syncNow} disabled={busy}>
              <span className="flex items-center gap-1.5">
                <RefreshCw size={14} /> Sync now
              </span>
            </Btn>
            <Btn variant="ghost" onClick={signOut} disabled={busy}>
              <span className="flex items-center gap-1.5">
                <LogOut size={14} /> Sign out
              </span>
            </Btn>
          </div>

          <div className="mt-4 border-t border-hairline pt-4">
            <Field label="Change password">
              <TextInput
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                placeholder="New password (min 6 characters)"
                onKeyDown={(e) => e.key === "Enter" && newPassword.length >= 6 && changePassword()}
              />
            </Field>
            <Btn variant="ghost" onClick={changePassword} disabled={busy || newPassword.length < 6}>
              Update password
            </Btn>
          </div>
        </>
      ) : (
        <>
          <p className="mb-4 text-[13px] leading-relaxed text-text-secondary">
            Sign in on each Mac with the same account to keep tasks and timers in sync.
          </p>
          <Field label="Email">
            <TextInput type="email" value={email} onChange={(e) => setEmail(e.target.value)} autoFocus />
          </Field>
          <Field label="Password">
            <TextInput
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && signIn()}
            />
          </Field>
          <div className="mt-2 flex gap-2">
            <Btn variant="primary" onClick={signIn} disabled={busy || !email || !password}>
              Sign in
            </Btn>
            <Btn variant="ghost" onClick={signUp} disabled={busy || !email || !password}>
              Create account
            </Btn>
          </div>
        </>
      )}
      {msg ? <div className="mt-3 text-[12px] text-text-secondary">{msg}</div> : null}
    </Modal>
  );
}
