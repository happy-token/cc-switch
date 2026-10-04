import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  cleanup,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HappyTokenLoginButton } from "@/components/HappyTokenLoginButton";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  invalidate: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  warning: vi.fn(),
  openExternal: vi.fn(),
  handlers: new Map<string, (payload?: unknown) => unknown>(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({ invalidateQueries: mocks.invalidate }),
}));
vi.mock("react-i18next", () => {
  const t = (key: string) => key;
  return { useTranslation: () => ({ t }) };
});
vi.mock("@/lib/api/settings", () => ({
  settingsApi: { openExternal: mocks.openExternal },
}));
vi.mock("@/lib/toast", () => ({
  toast: { success: mocks.success, error: mocks.error, warning: mocks.warning },
}));
vi.mock("@/hooks/useTauriEvent", () => ({
  useTauriEvent: (event: string, handler: (payload?: unknown) => unknown) => {
    mocks.handlers.set(event, handler);
  },
}));

beforeEach(() => {
  vi.clearAllMocks();
  mocks.handlers.clear();
  mocks.invoke.mockImplementation(async (command: string) =>
    command === "happy_token_account" ? null : { code: "1234ABCD" },
  );
  mocks.invalidate.mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("HappyToken login button", () => {
  it("opens browser login and allows retry after cancellation", async () => {
    render(<HappyTokenLoginButton />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button"));
    });
    expect(mocks.invoke).toHaveBeenCalledWith("happy_token_login");
    await waitFor(() =>
      expect(screen.getByText("1234ABCD")).toBeInTheDocument(),
    );
    act(() => {
      mocks.handlers.get("happy-token-cancelled")?.();
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button"));
    });
    expect(
      mocks.invoke.mock.calls.filter(
        ([command]) => command === "happy_token_login",
      ),
    ).toHaveLength(2);
  });

  it("refreshes providers and displays partial group failures after sync", async () => {
    render(<HappyTokenLoginButton />);
    act(() => {
      mocks.handlers.get("happy-token-syncing")?.();
    });
    expect(screen.getByRole("button")).toBeDisabled();
    await act(async () => {
      await mocks.handlers.get("happy-token-synced")?.({
        account: "Test",
        groups: ["default", "gpt-pro", "gpt-web"],
        providers: 3,
        warnings: ["image: no coding models"],
      });
    });
    expect(mocks.invalidate).toHaveBeenCalledWith({ queryKey: ["providers"] });
    expect(mocks.success).toHaveBeenCalledOnce();
    expect(mocks.warning).toHaveBeenCalledWith(
      "image: no coding models",
      expect.anything(),
    );
    expect(screen.getByRole("button")).toBeEnabled();
    expect(screen.getByRole("button")).toHaveTextContent("Test");
  });

  it("reports native login launch failure and resets the button", async () => {
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === "happy_token_login") throw new Error("launch failure");
      return null;
    });
    render(<HappyTokenLoginButton />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button"));
    });
    await waitFor(() =>
      expect(mocks.error).toHaveBeenCalledWith("happyToken.openFailed"),
    );
    expect(screen.getByRole("button")).toBeEnabled();
  });
  it("shows the matching code, reopens the same browser request and cancels", async () => {
    render(<HappyTokenLoginButton />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button"));
    });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("1234ABCD")).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: "happyToken.reopen" }),
      );
    });
    expect(
      mocks.invoke.mock.calls.filter(
        ([command]) => command === "happy_token_login",
      ),
    ).toHaveLength(2);
    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: "happyToken.cancel" }),
      );
    });
    expect(mocks.invoke).toHaveBeenLastCalledWith("happy_token_cancel_login");
    act(() => {
      mocks.handlers.get("happy-token-cancelled")?.();
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
  it("restores the account, shows amounts and opens recharge in the browser", async () => {
    mocks.invoke.mockResolvedValue({
      account: "Fixture",
      overview: {
        balance: 12.5,
        consumed: 3,
        symbol: "¥",
        updatedAt: 1700000000000,
      },
    });
    render(<HappyTokenLoginButton />);
    await waitFor(() =>
      expect(screen.getByRole("button")).toHaveTextContent("Fixture"),
    );
    fireEvent.click(screen.getByRole("button"));
    expect(await screen.findByText("¥12.50")).toBeInTheDocument();
    expect(screen.getByText("¥3.00")).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: "happyToken.recharge" }),
      );
    });
    expect(mocks.openExternal).toHaveBeenCalledWith(
      "https://gateway.happy-token.cn/sso?next=%2Fwallet&lang=zh",
    );
    expect(mocks.invoke).not.toHaveBeenCalledWith("happy_token_login");
    fireEvent.click(screen.getByRole("button"));
    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: "happyToken.console" }),
      );
    });
    expect(mocks.openExternal).toHaveBeenLastCalledWith(
      "https://gateway.happy-token.cn/sso?next=%2Fdashboard&lang=zh",
    );
  });
  it("keeps a collapsed account accessible and does not present missing amounts as zero", async () => {
    mocks.invoke.mockResolvedValue({ account: "Fixture", overview: null });
    render(<HappyTokenLoginButton collapsed />);
    const trigger = await screen.findByRole("button", {
      name: "happyToken.accountMenu",
    });
    expect(trigger).not.toHaveTextContent("Fixture");
    expect(trigger).toHaveAttribute("title", "Fixture");
    fireEvent.click(trigger);
    expect(
      await screen.findByText("happyToken.overviewUnavailable"),
    ).toBeInTheDocument();
    expect(screen.getAllByText("—")).toHaveLength(2);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "happyToken.sync" }));
    });
    expect(mocks.invoke).toHaveBeenCalledWith("happy_token_login");
  });
  it("does not replace a newly synchronized account with a delayed cached account", async () => {
    let restore!: (value: unknown) => void;
    mocks.invoke.mockImplementation(
      () =>
        new Promise((resolve) => {
          restore = resolve;
        }),
    );
    render(<HappyTokenLoginButton />);
    await act(async () => {
      await mocks.handlers.get("happy-token-synced")?.({
        account: "New account",
        groups: ["default"],
        providers: 1,
        warnings: [],
      });
    });
    await act(async () => {
      restore({ account: "Old account", overview: null });
    });
    expect(screen.getByRole("button")).toHaveTextContent("New account");
    expect(screen.queryByText("Old account")).not.toBeInTheDocument();
  });
  it("logs out locally, ignores late sync, and allows a new login", async () => {
    mocks.invoke.mockImplementation(async (command: string) =>
      command === "happy_token_account"
        ? { account: "Fixture", overview: null }
        : { code: "1234ABCD" },
    );
    render(<HappyTokenLoginButton />);
    await waitFor(() =>
      expect(screen.getByRole("button")).toHaveTextContent("Fixture"),
    );
    fireEvent.click(screen.getByRole("button"));
    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: "happyToken.logout" }),
      );
    });
    expect(mocks.invoke).toHaveBeenCalledWith("happy_token_logout");
    await act(async () => {
      await mocks.handlers.get("happy-token-synced")?.({
        account: "Late",
        groups: [],
        providers: 0,
        warnings: [],
      });
    });
    expect(screen.getByRole("button")).toHaveTextContent("happyToken.login");
    await act(async () => {
      fireEvent.click(screen.getByRole("button"));
    });
    expect(mocks.invoke).toHaveBeenCalledWith("happy_token_login");
  });
  it("keeps the account when logout fails", async () => {
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === "happy_token_logout")
        throw new Error("database unavailable");
      return { account: "Fixture", overview: null };
    });
    render(<HappyTokenLoginButton />);
    await waitFor(() =>
      expect(screen.getByRole("button")).toHaveTextContent("Fixture"),
    );
    fireEvent.click(screen.getByRole("button"));
    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: "happyToken.logout" }),
      );
    });
    expect(mocks.error).toHaveBeenCalledWith("happyToken.logoutFailed");
    expect(
      screen.getByRole("button", { name: "happyToken.accountMenu" }),
    ).toHaveTextContent("Fixture");
  });
});
