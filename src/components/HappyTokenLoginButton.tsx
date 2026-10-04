import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { KeyRound, Loader2 } from "lucide-react";
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

interface SyncResult {
  account: string;
  groups: string[];
  providers: number;
  warnings: string[];
}

export function HappyTokenLoginButton() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<"idle" | "login" | "syncing">("idle");
  const [account, setAccount] = useState("");
  const [code, setCode] = useState("");
  const [opening, setOpening] = useState(false);

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
    setAccount(result.account);
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

  const cancel = async () => {
    try {
      await invoke("happy_token_cancel_login");
    } catch {
      toast.error(t("happyToken.cancelFailed"));
    }
  };

  return (
    <>
      <Button
        variant="quiet"
        size="regular"
        disabled={status === "syncing" || opening}
        onClick={() => void login()}
        title={
          account ? t("happyToken.account", { account }) : t("happyToken.help")
        }
      >
        {status === "idle" ? (
          <KeyRound className="h-4 w-4" />
        ) : (
          <Loader2 className="h-4 w-4 animate-spin" />
        )}
        {t(
          status === "syncing"
            ? "happyToken.syncing"
            : account
              ? "happyToken.sync"
              : "happyToken.login",
        )}
      </Button>
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
