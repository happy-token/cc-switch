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
  handlers: new Map<string, (payload?: unknown) => unknown>(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({ invalidateQueries: mocks.invalidate }),
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
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
  mocks.invoke.mockResolvedValue({ code: "1234ABCD" });
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
    expect(mocks.invoke).toHaveBeenCalledTimes(2);
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
    expect(screen.getByRole("button")).toHaveTextContent("happyToken.sync");
  });

  it("reports native login launch failure and resets the button", async () => {
    mocks.invoke.mockRejectedValueOnce(new Error("launch failure"));
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
    expect(mocks.invoke).toHaveBeenCalledTimes(2);
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
});
