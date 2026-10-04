import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { ChevronUp, Loader2, UserRound } from "lucide-react";
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
      toast.warning(result.warnings.join("；"), { duration: 12000 });
    }
  });

  const login = async () => {
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
      maximumFractionDigits: 4,
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
      disabled={status === "syncing" || opening}
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

  return (
    <>
      {account ? (
        <Popover open={menuOpen} onOpenChange={setMenuOpen}>
          <PopoverTrigger asChild>{trigger}</PopoverTrigger>
          <PopoverContent
            side="top"
            align="start"
            sideOffset={8}
            className="w-64 max-w-[calc(100vw-16px)] p-3"
            aria-label={t("happyToken.accountMenu", { account })}
          >
            <p className="mb-3 break-words whitespace-normal font-medium">
              {account}
            </p>
            <dl className="space-y-2 text-body">
              <div className="flex justify-between gap-3">
                <dt className="text-fg-2">{t("happyToken.balance")}</dt>
                <dd className="tabular-nums">
                  {overview ? amount(overview.balance) : "—"}
                </dd>
              </div>
              <div className="flex justify-between gap-3">
                <dt className="text-fg-2">{t("happyToken.consumed")}</dt>
                <dd className="tabular-nums">
                  {overview ? amount(overview.consumed) : "—"}
                </dd>
              </div>
            </dl>
            <p className="my-3 whitespace-normal text-caption text-fg-3">
              {overview
                ? t("happyToken.updatedAt", {
                    time: new Date(overview.updatedAt).toLocaleString(),
                  })
                : t("happyToken.overviewUnavailable")}
            </p>
            <div className="flex gap-2">
              <Button
                variant="outline"
                size="compact"
                className="flex-1"
                onClick={() => void openAccountPage("/wallet")}
              >
                {t("happyToken.recharge")}
              </Button>
              <Button
                variant="outline"
                size="compact"
                className="flex-1"
                onClick={() => void openAccountPage("/dashboard")}
              >
                {t("happyToken.console")}
              </Button>
            </div>
            <Button
              variant="quiet"
              size="compact"
              className="mt-2 w-full"
              disabled={status !== "idle" || opening}
              onClick={() => void login()}
            >
              {t("happyToken.sync")}
            </Button>
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
          onInteractOutside={(event) => {
            if (status === "syncing") event.preventDefault();
          }}
          onEscapeKeyDown={(event) => {
            if (status === "syncing") event.preventDefault();
          }}
        >
          <DialogHeader>
            <DialogTitle>{t("happyToken.browserTitle")}</DialogTitle>
            <DialogDescription>{t("happyToken.browserHelp")}</DialogDescription>
          </DialogHeader>
          <p
            className="text-center font-mono text-3xl tracking-widest py-4"
            aria-label={t("happyToken.code")}
          >
            {code}
          </p>
          <p className="text-sm text-muted-foreground">
            {t(
              status === "syncing"
                ? "happyToken.syncing"
                : "happyToken.waiting",
            )}
          </p>
          <div className="flex justify-end gap-2">
            <Button
              variant="outline"
              disabled={status === "syncing" || opening}
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
