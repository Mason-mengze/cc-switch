import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ProviderActions } from "@/components/providers/ProviderActions";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) =>
      options?.defaultValue ?? key,
  }),
}));

describe("ProviderActions", () => {
  it("uses remove-from-config instead of delete for vscode-copilot additive items", () => {
    const onRemoveFromConfig = vi.fn();
    const onDelete = vi.fn();

    render(
      <ProviderActions
        appId="vscode-copilot"
        isCurrent={false}
        isInConfig={true}
        onSwitch={vi.fn()}
        onEdit={vi.fn()}
        onDuplicate={vi.fn()}
        onConfigureUsage={vi.fn()}
        onDelete={onDelete}
        onRemoveFromConfig={onRemoveFromConfig}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "移除" }));

    expect(onRemoveFromConfig).toHaveBeenCalledTimes(1);
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("keeps the trash button wired to permanent delete", () => {
    const onDelete = vi.fn();

    render(
      <ProviderActions
        appId="vscode-copilot"
        isCurrent={false}
        isInConfig={true}
        onSwitch={vi.fn()}
        onEdit={vi.fn()}
        onDuplicate={vi.fn()}
        onConfigureUsage={vi.fn()}
        onDelete={onDelete}
        onRemoveFromConfig={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByTitle("common.delete"));

    expect(onDelete).toHaveBeenCalledTimes(1);
  });
});
