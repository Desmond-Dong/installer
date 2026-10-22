import { expect, fixture, html } from "@open-wc/testing";
import "../../../../src/views/minipc/setup-method-view.js";
import type { MiniPCSetupMethodView } from "../../../../src/views/minipc/setup-method-view.js";
import type { InfoDialog } from "../../../../src/components/info-dialog.js";
import type { OptionCard } from "../../../../src/components/option-card.js";
import { wizardState } from "../../../../src/state/wizard-state.js";
import { captureOpenedUrls, restoreTauriIpc, settle } from "../../tauri-ipc.js";

async function renderMethod() {
  wizardState.startFlow("minipc");
  const el = await fixture<MiniPCSetupMethodView>(
    html`<minipc-setup-method-view></minipc-setup-method-view>`
  );
  const [usbInstaller, connectDrive, usbBoot] =
    el.shadowRoot!.querySelectorAll<OptionCard>("option-card");
  const dialog = el.shadowRoot!.querySelector<InfoDialog>("info-dialog")!;
  return { el, usbInstaller, connectDrive, usbBoot, dialog };
}

/** The dialog's footer button with the given label. */
function dialogButton(dialog: InfoDialog, label: string) {
  return [...dialog.shadowRoot!.querySelectorAll("wa-button")].find(
    (button) => button.textContent!.trim() === label
  )!;
}

/**
 * The dialog closes itself on either button; only the view forgetting it was
 * shown lets the next USB boot click show it again.
 */
async function expectReopens(
  el: MiniPCSetupMethodView,
  usbBoot: OptionCard,
  dialog: InfoDialog
) {
  usbBoot.click();
  await el.updateComplete;
  expect(dialog.open).to.be.true;
}

describe("minipc-setup-method-view", () => {
  afterEach(() => {
    restoreTauriIpc();
    wizardState.reset();
  });

  it("offers connecting the drive or booting from USB", async () => {
    const { el, usbInstaller, connectDrive, usbBoot, dialog } =
      await renderMethod();
    expect(el.shadowRoot!.querySelector("h2")!.textContent).to.contain(
      "How will you install?"
    );
    expect(usbInstaller.heading).to.equal("Install from a USB stick");
    expect(usbInstaller.description).to.contain("bootable installer");
    expect(connectDrive.heading).to.equal("I can connect the drive");
    expect(connectDrive.description).to.contain("SSD or NVMe");
    expect(usbBoot.heading).to.equal("I need to boot from USB");
    expect(usbBoot.description).to.contain("bootable USB drive");
    expect(dialog.open).to.be.false;
  });

  it("starts the x86 USB installer and skips the architecture step", async () => {
    const { usbInstaller } = await renderMethod();
    usbInstaller.click();
    expect(wizardState.currentStep!.id).to.equal("drive");
    const { selections } = wizardState.getState();
    expect(selections.deviceConfig?.board).to.equal("x86-installer");
    expect(selections.deviceConfig?.minimum_storage_bytes).to.equal(
      1_000_000_000
    );
    // The installer ISO only boots on x86, and such an installer has no
    // built-in Wi-Fi: the wizard hides the Wi-Fi step for it.
    expect(
      wizardState.getState().steps.map((step) => step.id)
    ).to.not.contain("wifi");
  });

  it("continues to the architecture step when the drive can be connected", async () => {
    const { connectDrive, dialog } = await renderMethod();
    connectDrive.click();
    expect(wizardState.currentStep!.id).to.equal("architecture");
    expect(dialog.open).to.be.false;
  });

  it("explains that USB boot is unsupported and stays on this step", async () => {
    const { el, usbBoot, dialog } = await renderMethod();
    usbBoot.click();
    await el.updateComplete;
    expect(dialog.open).to.be.true;
    expect(dialog.heading).to.equal("USB boot installation");
    expect(dialog.message).to.contain("not supported by this installer");
    expect(wizardState.currentStep!.id).to.equal("method");
  });

  it("closes the explanation on Go back without leaving the step", async () => {
    const opened = captureOpenedUrls();
    const { el, usbBoot, dialog } = await renderMethod();
    usbBoot.click();
    await el.updateComplete;
    dialogButton(dialog, "Go back").click();
    await el.updateComplete;
    await settle();
    expect(dialog.open).to.be.false;
    expect(opened).to.deep.equal([]);
    expect(wizardState.currentStep!.id).to.equal("method");
    await expectReopens(el, usbBoot, dialog);
  });

  it("opens the generic x86-64 guide from the explanation", async () => {
    const opened = captureOpenedUrls();
    const { el, usbBoot, dialog } = await renderMethod();
    usbBoot.click();
    await el.updateComplete;
    dialogButton(dialog, "View instructions").click();
    await el.updateComplete;
    await settle();
    expect(dialog.open).to.be.false;
    expect(opened).to.deep.equal([
      "https://www.home-assistant.io/installation/generic-x86-64",
    ]);
    expect(wizardState.currentStep!.id).to.equal("method");
    await expectReopens(el, usbBoot, dialog);
  });
});
