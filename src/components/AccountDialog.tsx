import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../api";
import { Icon } from "./Icon";
import type { AccountInfo, MicrosoftDeviceCode, NoticeKind } from "../types";
import { formatUnixSeconds, toErrorMessage } from "../utils";

interface AccountDialogProps {
  accounts: AccountInfo[];
  accountsLoading: boolean;
  accountsError: string | null;
  microsoftClientId: string;
  onClose: () => void;
  onOpenSettings: () => void;
  onAccountsChanged: (accounts: AccountInfo[]) => void;
  onRefreshAccounts: () => Promise<void>;
  onNotify: (kind: NoticeKind, message: string) => void;
}

export function AccountDialog({
  accounts,
  accountsLoading,
  accountsError,
  microsoftClientId,
  onClose,
  onOpenSettings,
  onAccountsChanged,
  onRefreshAccounts,
  onNotify
}: AccountDialogProps) {
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [startingLogin, setStartingLogin] = useState(false);
  const [deviceFlow, setDeviceFlow] = useState<MicrosoftDeviceCode | null>(null);
  const [loginStatus, setLoginStatus] = useState("");
  const [loginError, setLoginError] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState("");
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const dialogRef = useRef<HTMLElement>(null);
  const mountedRef = useRef(true);
  const onCloseRef = useRef(onClose);

  const activeAccount = accounts.find((account) => account.active) ?? null;
  const microsoftReady = Boolean(microsoftClientId.trim());

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    mountedRef.current = true;
    closeButtonRef.current?.focus();
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab" || !dialogRef.current) return;
      const focusable = Array.from(dialogRef.current.querySelectorAll<HTMLElement>("button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href]")).filter((element) => element.offsetParent !== null);
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && (document.activeElement === first || !dialogRef.current.contains(document.activeElement))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => {
      mountedRef.current = false;
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);
  useEffect(() => {
    if (!deviceFlow) return;
    const flow = deviceFlow;
    let cancelled = false;
    let timer: number | undefined;
    let intervalMs = Math.max(1, flow.interval) * 1000;

    async function poll() {
      try {
        const result = await api.pollMicrosoftLogin(flow.deviceCode);
        if (cancelled) return;
        setLoginStatus(result.message);
        if (result.status === "success") {
          setDeviceFlow(null);
          setLoginError(null);
          await onRefreshAccounts();
          if (mountedRef.current) onNotify("success", "Microsoft 账户登录成功，并已设为当前账户。");
          return;
        }
        if (result.status === "error") {
          setDeviceFlow(null);
          setLoginError(result.message || "Microsoft 登录失败。");
          return;
        }
        if (result.slowDown) intervalMs = Math.min(intervalMs + 5000, 60_000);
        timer = window.setTimeout(() => void poll(), intervalMs);
      } catch (error) {
        if (cancelled) return;
        setDeviceFlow(null);
        setLoginError(`登录轮询失败：${toErrorMessage(error)}`);
      }
    }

    timer = window.setTimeout(() => void poll(), intervalMs);
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [deviceFlow, onNotify, onRefreshAccounts]);

  async function beginLogin() {
    if (!microsoftReady || startingLogin) return;
    setStartingLogin(true);
    setActionError(null);
    setLoginError(null);
    setCopyStatus("");
    try {
      const flow = await api.beginMicrosoftLogin();
      if (!mountedRef.current) return;
      setDeviceFlow(flow);
      setLoginStatus(flow.message || "等待在浏览器中完成授权。");
    } catch (error) {
      if (mountedRef.current) setLoginError(`无法开始 Microsoft 登录：${toErrorMessage(error)}`);
    } finally {
      if (mountedRef.current) setStartingLogin(false);
    }
  }

  async function selectAccount(account: AccountInfo) {
    if (account.active || busyAction) return;
    setBusyAction(`select:${account.id}`);
    setActionError(null);
    try {
      const updated = await api.selectAccount(account.id);
      onAccountsChanged(updated);
      if (mountedRef.current) onNotify("success", `已切换到 ${account.username}。`);
    } catch (error) {
      if (mountedRef.current) setActionError(`切换账户失败：${toErrorMessage(error)}`);
    } finally {
      if (mountedRef.current) setBusyAction(null);
    }
  }
  async function removeAccount(account: AccountInfo) {
    if (account.accountType === "offline" || busyAction) return;
    if (!window.confirm(`确定移除 Microsoft 账户 ${account.username} 吗？`)) return;
    setBusyAction(`remove:${account.id}`);
    setActionError(null);
    try {
      const updated = await api.removeAccount(account.id);
      onAccountsChanged(updated);
      if (mountedRef.current) onNotify("success", `已移除账户 ${account.username}。`);
    } catch (error) {
      if (mountedRef.current) setActionError(`移除账户失败：${toErrorMessage(error)}`);
    } finally {
      if (mountedRef.current) setBusyAction(null);
    }
  }
  async function refreshActive() {
    if (refreshing || busyAction) return;
    setRefreshing(true);
    setActionError(null);
    try {
      const refreshed = await api.refreshActiveAccount();
      await onRefreshAccounts();
      if (mountedRef.current) onNotify("success", `已刷新账户 ${refreshed.username} 的登录信息。`);
    } catch (error) {
      if (mountedRef.current) setActionError(`刷新账户失败：${toErrorMessage(error)}`);
    } finally {
      if (mountedRef.current) setRefreshing(false);
    }
  }

  async function copyToClipboard(value: string, successMessage: string) {
    try {
      let copied = false;
      if (navigator.clipboard?.writeText) {
        try {
          await navigator.clipboard.writeText(value);
          copied = true;
        } catch {
          copied = false;
        }
      }
      if (!copied) {
        const textarea = document.createElement("textarea");
        textarea.value = value;
        textarea.style.position = "fixed";
        textarea.style.opacity = "0";
        document.body.appendChild(textarea);
        try {
          textarea.select();
          copied = document.execCommand("copy");
        } finally {
          textarea.remove();
        }
      }
      if (!copied) throw new Error("当前 WebView 不支持剪贴板写入。");
      if (mountedRef.current) setCopyStatus(successMessage);
    } catch (error) {
      if (mountedRef.current) setCopyStatus(`复制失败：${toErrorMessage(error)}`);
    }
  }

  async function copyUserCode() {
    if (deviceFlow) await copyToClipboard(deviceFlow.userCode, "用户代码已复制。");
  }

  async function copyVerificationLink() {
    if (deviceFlow) await copyToClipboard(deviceFlow.verificationUri, "验证地址已复制。");
  }

  async function openVerificationPage() {
    if (!deviceFlow) return;
    try {
      await openUrl(deviceFlow.verificationUri);
    } catch (error) {
      if (mountedRef.current) setLoginError(`无法打开验证页面：${toErrorMessage(error)}`);
    }
  }

  const accountActionsDisabled = busyAction !== null || refreshing || startingLogin;
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section ref={dialogRef} className="modal account-dialog" role="dialog" aria-modal="true" aria-labelledby="account-dialog-title">
        <header className="modal-head">
          <div>
            <h2 id="account-dialog-title">账户管理</h2>
            <span>离线账户与 Microsoft 登录</span>
          </div>
          <button ref={closeButtonRef} type="button" className="modal-close" onClick={onClose} aria-label="关闭账户管理" title="关闭">
            <Icon name="close" />
          </button>
        </header>

        <div className="modal-body account-body">
          {accountsError ? (
            <div className="inline-message error" role="alert">
              <Icon name="alert" />
              <span>{accountsError}</span>
              <button type="button" className="text-link" onClick={() => void onRefreshAccounts()}>重试</button>
            </div>
          ) : null}
          {actionError ? (
            <div className="inline-message error" role="alert">
              <Icon name="alert" />
              <span>{actionError}</span>
            </div>
          ) : null}

          <div className="account-section-head">
            <span>账户列表</span>
            <button type="button" className="link-button" onClick={refreshActive} disabled={!activeAccount || refreshing || Boolean(busyAction)}>
              <Icon name="refresh" className={refreshing ? "icon-spin" : undefined} /> 刷新当前账户
            </button>
          </div>

          {accountsLoading && accounts.length === 0 ? (
            <div className="inline-loading"><span className="spinner" /> 正在读取账户...</div>
          ) : accounts.length === 0 ? (
            <div className="empty-line">没有可用账户。</div>
          ) : (
            <div className="account-list">
              {accounts.map((account) => (
                <div key={account.id} className={`account-row${account.active ? " active" : ""}`} aria-busy={busyAction === `select:${account.id}` || busyAction === `remove:${account.id}` || undefined}>
                  <span className="account-avatar" aria-hidden="true">{account.username.trim().slice(0, 1).toUpperCase() || "?"}</span>
                  <span className="account-row-copy">
                    <strong>{account.username}</strong>
                    <small>
                      {account.accountType === "microsoft" ? "Microsoft 账户" : "离线账户"}
                      {account.accountType === "microsoft" && account.expiresAt ? ` · 有效至 ${formatUnixSeconds(account.expiresAt)}` : ""}
                    </small>
                  </span>
                  {account.active ? <span className="account-active-tag">当前</span> : null}
                  <span className="account-row-actions">
                    {!account.active ? (
                      <button type="button" className="button compact" onClick={() => void selectAccount(account)} disabled={accountActionsDisabled} aria-label={`使用账户 ${account.username}`}>
                        {busyAction === `select:${account.id}` ? <span className="spinner spinner-small" aria-hidden="true" /> : "使用"}
                      </button>
                    ) : null}
                    {account.accountType === "microsoft" ? (
                      <button type="button" className="icon-button danger" onClick={() => void removeAccount(account)} disabled={accountActionsDisabled || busyAction === `remove:${account.id}`} aria-label={`移除账户 ${account.username}`} title="移除账户">
                        {busyAction === `remove:${account.id}` ? <span className="spinner spinner-small" aria-hidden="true" /> : <Icon name="trash" />}
                      </button>
                    ) : null}
                  </span>
                </div>
              ))}
            </div>
          )}
          <div className="account-note">
            <span>离线账户用户名与 Microsoft 应用 ID 在设置中维护。</span>
            <button type="button" className="text-link" onClick={onOpenSettings}>打开设置</button>
          </div>
          <div className="account-microsoft">
            <div className="account-section-head">
              <span>Microsoft 登录</span>
              <span className="account-mode-label">设备代码流程</span>
            </div>

            {!microsoftReady ? (
              <div className="integration-warning">
                <Icon name="info" />
                <div>
                  <strong>尚未配置 Microsoft 应用程序 ID</strong>
                  <p>在设置中填写 Azure 公共客户端应用 ID 后，才能开始 Microsoft 登录。</p>
                </div>
                <button type="button" className="button compact" onClick={onOpenSettings}>前往设置</button>
              </div>
            ) : deviceFlow ? (
              <div className="device-code-panel">
                <p>{deviceFlow.message || loginStatus}</p>
                <div className="device-code-line">
                  <span>用户代码</span>
                  <code>{deviceFlow.userCode}</code>
                  <button type="button" className="button compact" onClick={() => void copyUserCode()}>
                    <Icon name="copy" /> 复制
                  </button>
                </div>
                <div className="device-uri-line">
                  <span>验证地址</span>
                  <code title={deviceFlow.verificationUri}>{deviceFlow.verificationUri}</code>
                  <button type="button" className="button compact" onClick={() => void copyVerificationLink()}><Icon name="copy" /> 复制链接</button>
                </div>
                <div className="device-actions">
                  <button type="button" className="button primary compact" onClick={() => void openVerificationPage()}>
                    <Icon name="open" /> 打开验证页面
                  </button>
                  <button type="button" className="button compact" onClick={() => { setDeviceFlow(null); setLoginStatus(""); setLoginError(null); }}>
                    取消登录
                  </button>
                  <span className="device-status">{copyStatus || loginStatus || "等待授权完成..."}</span>
                </div>
                <small className="field-help">代码有效期约 {Math.max(0, Math.floor(deviceFlow.expiresIn / 60))} 分钟；初始轮询间隔 {deviceFlow.interval} 秒。</small>
              </div>
            ) : (
              <div className="device-login-row">
                <div>
                  <strong>使用 Microsoft 账户登录</strong>
                  <p>将打开浏览器完成设备代码授权；Flint 不保存客户端密钥，也不会显示访问令牌。</p>
                </div>
                <button type="button" className="button primary compact" onClick={() => void beginLogin()} disabled={startingLogin || accountActionsDisabled}>
                  {startingLogin ? <span className="spinner spinner-light" /> : <Icon name="user" />}
                  {startingLogin ? "正在启动" : "开始登录"}
                </button>
              </div>
            )}

            {loginError ? (
              <div className="inline-message error compact" role="alert">
                <Icon name="alert" /><span>{loginError}</span>
              </div>
            ) : null}
          </div>
        </div>

        <footer className="modal-foot">
          <span>账户令牌由 Flint 核心保存在本机；界面只显示账户摘要。</span>
          <button type="button" className="button compact" onClick={onClose}>关闭</button>
        </footer>
      </section>
    </div>
  );
}