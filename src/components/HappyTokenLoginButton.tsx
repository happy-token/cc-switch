import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { KeyRound, Loader2 } from "lucide-react";
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

  useTauriEvent("happy-token-syncing", () => setStatus("syncing"));
  useTauriEvent("happy-token-cancelled", () => setStatus("idle"));
  useTauriEvent<string>("happy-token-error", (message) => {
    setStatus("idle");
    toast.error(message, { duration: 8000 });
  });
  useTauriEvent<SyncResult>("happy-token-synced", async (result) => {
    setStatus("idle");
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
    try {
      await invoke("happy_token_login");
    } catch {
      setStatus("idle");
      toast.error(t("happyToken.openFailed"));
    }
  };

  return (
    <Button
      variant="quiet"
      size="regular"
      disabled={status === "syncing"}
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
  );
}
