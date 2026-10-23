import { expect, fixture, html } from "@open-wc/testing";
import "../../../../src/views/minipc/setup-method-view.js";
import type { MiniPCSetupMethodView } from "../../../../src/views/minipc/setup-method-view.js";
import type { OptionCard } from "../../../../src/components/option-card.js";
import { wizardState } from "../../../../src/state/wizard-state.js";
import { restoreTauriIpc } from "../../tauri-ipc.js";

async function renderMethod() {
  wizardState.startFlow("minipc");
  const el = await fixture<MiniPCSetupMethodView>(
    html`<minipc-setup-method-view></minipc-setup-method-view>`
  );
  const [connectDrive, usbBoot] =
    el.shadowRoot!.querySelectorAll<OptionCard>("option-card");
  return { el, connectDrive, usbBoot };
}

describe("minipc-setup-method-view", () => {
  afterEach(() => {
    restoreTauriIpc();
    wizardState.reset();
  });

  it("offers connecting the drive or booting from USB", async () => {
    const { el, connectDrive, usbBoot } = await renderMethod();
    expect(el.shadowRoot!.querySelector("h2")!.textContent).to.contain(
      "How will you install?"
    );
    expect(connectDrive.heading).to.equal("I can connect the drive");
    expect(connectDrive.description).to.contain("SSD or NVMe");
    expect(usbBoot.heading).to.equal("I need to boot from USB");
    expect(usbBoot.description).to.contain("bootable USB drive");
  });

  it("starts the x86 USB installer and skips the architecture step", async () => {
    const { usbBoot } = await renderMethod();
    usbBoot.click();
    expect(wizardState.currentStep!.id).to.equal("drive");
    const { selections } = wizardState.getState();
    expect(selections.deviceConfig?.board).to.equal("x86-installer");
    expect(selections.deviceConfig?.minimum_storage_bytes).to.equal(
      1_000_000_000
    );
    // The installer ISO only boots on x86, and such an installer has no
    // built-in Wi-Fi: the wizard hides the Wi-Fi step for it.
    expect(wizardState.getState().steps.map((step) => step.id)).to.not.contain(
      "wifi"
    );
  });

  it("continues to the architecture step when the drive can be connected", async () => {
    const { connectDrive } = await renderMethod();
    connectDrive.click();
    expect(wizardState.currentStep!.id).to.equal("architecture");
  });
});
