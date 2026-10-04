import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  ChevronUp,
  Loader2,
  UserRound,
  ExternalLink,
  RefreshCw,
  LogOut,
  ShieldCheck,
  Plus,
} from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { toast } from "@/lib/toast";
import { settingsApi } from "@/lib/api/settings";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";

interface AccountSummary {
  account: string;
  overview?: {
    balance: number;
    consumed: number;
    symbol: string;
    updatedAt: number;
  } | null;
}
interface SyncResult extends AccountSummary {
  groups: string[];
  providers: number;
  warnings: string[];
}

// Older Workers still return verbose messages; normalize them at the display boundary.
export function compactSyncWarnings(warnings: string[]): string {
  const messages = new Set<string>();
  const routed = new Set<string>();
  for (const warning of warnings) {
    const separator = warning.indexOf("：");
    const group = separator < 0 ? "" : warning.slice(0, separator);
    const detail = separator < 0 ? warning : warning.slice(separator + 1);
    if (
      group === "gpt-web" &&
      detail.includes("不支持") &&
      detail.includes("工具")
    ) {
      messages.add("GPT Web 暂不支持编程助手");
    } else if (group === "image" && detail.includes("排除")) {
      messages.add("已跳过 Image");
    } else if (detail.includes("未声明 Gemini 原生接口")) {
      messages.add(`${group}：无 Gemini 接口，已跳过 Gemini CLI`);
    } else if (detail.includes("部分配置需要选择路由模式")) {
      routed.add(group);
    } else if (detail.includes("的已有协议设置保留")) {
      messages.add(
        `${group}：${detail.split(" 的已有协议设置保留")[0]} 协议需核对`,
      );
    } else {
      // Keep unexpected errors intact so partial sync failures remain visible.
      messages.add(warning);
    }
  }
  if (routed.size)
    messages.add(`${[...routed].join("、")}：部分配置需启用路由`);
  return [...messages].join("；");
}

export function HappyTokenLoginButton({
  collapsed = false,
}: {
  collapsed?: boolean;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<"idle" | "login" | "syncing">("idle");
  const [summary, setSummary] = useState<AccountSummary | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const synced = useRef(false);
  const loggedOut = useRef(false);
  const [loggingOut, setLoggingOut] = useState(false);
  const account = summary?.account || "";
  const [code, setCode] = useState("");
  const [opening, setOpening] = useState(false);

  useEffect(() => {
    let mounted = true;
    void invoke<AccountSummary | null>("happy_token_account")
      .then((saved) => {
        if (mounted && !synced.current) setSummary(saved);
      })
      .catch(() => {
        if (mounted) toast.error(t("happyToken.accountLoadFailed"));
      });
    return () => {
      mounted = false;
    };
  }, [t]);

  useTauriEvent("happy-token-syncing", () => setStatus("syncing"));
  useTauriEvent("happy-token-cancelled", () => {
    setStatus("idle");
    setCode("");
  });
  useTauriEvent<string>("happy-token-error", (message) => {
    setStatus("idle");
    setCode("");
    toast.error(message, { duration: 8000 });
  });
  useTauriEvent<SyncResult>("happy-token-synced", async (result) => {
    if (loggedOut.current) return;
    setStatus("idle");
    setCode("");
    synced.current = true;
    setSummary({ account: result.account, overview: result.overview });
    await queryClient.invalidateQueries({ queryKey: ["providers"] });
    toast.success(
      t("happyToken.synced", {
        count: result.providers,
        groups: result.groups.join("、"),
      }),
      { duration: 8000 },
    );
    if (result.warnings.length) {
      toast.warning(compactSyncWarnings(result.warnings), { duration: 8000 });
    }
  });

  const logout = async () => {
    setLoggingOut(true);
    try {
      await invoke("happy_token_logout");
      loggedOut.current = true;
      synced.current = true;
      setSummary(null);
      setMenuOpen(false);
      setStatus("idle");
      setCode("");
      toast.success(t("happyToken.loggedOut"));
    } catch {
      toast.error(t("happyToken.logoutFailed"));
    } finally {
      setLoggingOut(false);
    }
  };

  const login = async () => {
    loggedOut.current = false;
    setMenuOpen(false);
    setStatus("login");
    setOpening(true);
    try {
      const result = await invoke<{ code: string }>("happy_token_login");
      setCode(result.code);
    } catch {
      setStatus("idle");
      setCode("");
      toast.error(t("happyToken.openFailed"));
    } finally {
      setOpening(false);
    }
  };

  const openAccountPage = async (path: "/dashboard" | "/wallet") => {
    try {
      await settingsApi.openExternal(
        `https://gateway.happy-token.cn/sso?next=${encodeURIComponent(path)}&lang=zh`,
      );
      setMenuOpen(false);
    } catch {
      toast.error(t("happyToken.linkFailed"));
    }
  };

  const overview = summary?.overview;
  const amount = (value: number) =>
    `${overview?.symbol}${new Intl.NumberFormat(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(value)}`;
  const label =
    status === "syncing"
      ? t("happyToken.syncing")
      : account || t("happyToken.login");
  const trigger = (
    <Button
      variant="quiet"
      size="regular"
      className={cn(
        "w-full min-w-0 px-2",
        collapsed ? "justify-center" : "justify-start",
      )}
      disabled={status === "syncing" || opening || loggingOut}
      onClick={!account ? () => void login() : undefined}
      title={account || t("happyToken.help")}
      aria-label={account ? t("happyToken.accountMenu", { account }) : label}
    >
      {status === "idle" ? (
        <UserRound className="h-4 w-4 shrink-0" />
      ) : (
        <Loader2 className="h-4 w-4 shrink-0 animate-spin" />
      )}
      {!collapsed && (
        <span className="min-w-0 flex-1 truncate text-start">{label}</span>
      )}
      {!collapsed && account && <ChevronUp className="h-3.5 w-3.5 shrink-0" />}
    </Button>
  );

  const cancel = async () => {
    try {
      await invoke("happy_token_cancel_login");
    } catch {
      toast.error(t("happyToken.cancelFailed"));
    }
  };

  const accountDetails = (
    <>
      <div className="flex items-center gap-3 border-b border-border px-5 py-4">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-subtle text-fg-2">
          <UserRound className="h-5 w-5" />
        </div>
        <div className="min-w-0">
          <p className="text-caption text-fg-3">HappyToken</p>
          <p className="break-words font-medium text-fg-1">{account}</p>
        </div>
      </div>
      <div className="px-5 py-4">
        <dl>
          <div>
            <dt className="text-caption text-fg-2">
              {t("happyToken.balance")}
            </dt>
            <dd className="mt-1 text-2xl font-semibold tracking-tight tabular-nums">
              {overview ? amount(overview.balance) : "—"}
            </dd>
          </div>
          <div className="mt-3 flex items-center justify-between gap-3 text-caption">
            <dt className="text-fg-2">{t("happyToken.consumed")}</dt>
            <dd className="tabular-nums text-fg-1">
              {overview ? amount(overview.consumed) : "—"}
            </dd>
          </div>
        </dl>
        <p className="mt-3 whitespace-normal text-caption leading-relaxed text-fg-3">
          {overview
            ? t("happyToken.updatedAt", {
                time: new Date(overview.updatedAt).toLocaleString(),
              })
            : t("happyToken.overviewUnavailable")}
        </p>
        <div className="mt-4 grid grid-cols-1 gap-2">
          <Button
            variant="solid"
            size="regular"
            onClick={() => void openAccountPage("/wallet")}
          >
            <Plus className="h-3.5 w-3.5" />
            {t("happyToken.recharge")}
          </Button>
          <Button
            variant="outline"
            size="regular"
            onClick={() => void openAccountPage("/dashboard")}
          >
            <ExternalLink className="h-3.5 w-3.5" />
            {t("happyToken.console")}
          </Button>
        </div>
      </div>
      <div className="border-t border-border p-2">
        <Button
          variant="quiet"
          size="regular"
          className="w-full justify-start"
          disabled={status !== "idle" || opening || loggingOut}
          onClick={() => void login()}
        >
          <RefreshCw className="h-4 w-4 text-fg-2" />
          {t("happyToken.sync")}
        </Button>
        <Button
          variant="quiet"
          size="regular"
          className="w-full justify-start"
          disabled={status !== "idle" || opening || loggingOut}
          onClick={() => void logout()}
        >
          <LogOut className="h-4 w-4 text-fg-2" />
          {t("happyToken.logout")}
        </Button>
      </div>
    </>
  );

  return (
    <>
      {account ? (
        <Popover open={menuOpen} onOpenChange={setMenuOpen}>
          <PopoverTrigger asChild>{trigger}</PopoverTrigger>
          <PopoverContent
            side="top"
            align="start"
            sideOffset={8}
            className="w-72 max-w-[calc(100vw-16px)] overflow-hidden p-0"
            aria-label={t("happyToken.accountMenu", { account })}
          >
            {accountDetails}
          </PopoverContent>
        </Popover>
      ) : (
        trigger
      )}
      <Dialog
        open={status !== "idle" && !!code}
        onOpenChange={(open) => {
          if (!open && status === "login") void cancel();
        }}
      >
        <DialogContent
          className="max-w-md gap-5 p-7"
          onInteractOutside={(event) => {
            if (status === "syncing") event.preventDefault();
          }}
          onEscapeKeyDown={(event) => {
            if (status === "syncing") event.preventDefault();
          }}
        >
          <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-subtle">
            <ShieldCheck className="h-6 w-6 text-fg-1" />
          </div>
          <DialogHeader className="p-0">
            <DialogTitle>{t("happyToken.browserTitle")}</DialogTitle>
            <DialogDescription>{t("happyToken.browserHelp")}</DialogDescription>
          </DialogHeader>
          <div className="rounded-panel border border-border bg-subtle px-4 py-5 text-center">
            <p className="mb-3 text-caption text-fg-2">
              {t("happyToken.code")}
            </p>
            <p
              className="font-mono text-3xl tracking-[0.22em] text-fg-1"
              aria-label={t("happyToken.code")}
            >
              {code}
            </p>
          </div>
          <p
            className="flex items-center gap-2 text-caption text-fg-2"
            role="status"
          >
            <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin" />
            {t(
              status === "syncing"
                ? "happyToken.syncing"
                : "happyToken.waiting",
            )}
          </p>
          <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-4">
            <Button
              variant="solid"
              disabled={status === "syncing" || opening || loggingOut}
              onClick={() => void login()}
            >
              {t("happyToken.reopen")}
            </Button>
            <Button
              variant="outline"
              disabled={status === "syncing"}
              onClick={() => void cancel()}
            >
              {t("happyToken.cancel")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
