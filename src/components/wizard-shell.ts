import { localize } from "../localization/localize.js";
import { LitElement, html, css, nothing } from "lit";
import { customElement, property, state } from "lit/decorators.js";
import {
  wizardState,
  type WizardState,
  type WizardStep,
} from "../state/wizard-state.js";
import "@home-assistant/webawesome/dist/components/button/button.js";
import "./step-indicator.js";

@customElement("wizard-shell")
export class WizardShell extends LitElement {
  static styles = css`
    :host {
      display: flex;
      flex-direction: column;
      height: 100%;
    }

    .header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 1rem 2rem;
      border-bottom: 1px solid var(--ha-border-color, #e0e0e0);
    }

    @media (prefers-color-scheme: dark) {
      .header {
        border-bottom-color: var(--ha-border-color, #333333);
      }
    }

    .header-center {
      flex: 1;
      min-width: 0;
      display: flex;
      justify-content: center;
    }

    .header-right {
      min-width: 80px;
    }

    @media (max-width: 600px) {
      .header {
        padding: 1rem;
      }
      .header-right {
        min-width: 0;
      }
    }

    .content {
      flex: 1;
      overflow-y: auto;
      padding: 2rem;
    }

    /* The step titles are headings below this one; the steps show the flow */
    .flow-title {
      position: absolute;
      width: 1px;
      height: 1px;
      margin: -1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    .footer {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 1rem 2rem;
      border-top: 1px solid var(--ha-border-color, #e0e0e0);
    }

    .footer-left {
      display: flex;
      gap: 1rem;
    }

    .footer-right {
      display: flex;
      gap: 1rem;
    }

    @media (prefers-color-scheme: dark) {
      .footer {
        border-top-color: var(--ha-border-color, #333333);
      }
    }
  `;

  @state()
  private _wizardState: WizardState = wizardState.getState();

  /** Whether the step is taller than the window, so it can be scrolled. */
  @state()
  private _scrolls = false;

  private _resizeObserver = new ResizeObserver(() => this._checkScrolling());

  /** What is being installed, as the page's main heading. */
  @property({ type: String })
  flowTitle = "";

  @property({ type: String })
  nextLabel = localize("common.next");

  @property({ type: Boolean })
  nextDisabled = false;

  @property({ type: Boolean })
  hideBack = false;

  @property({ type: Boolean })
  hideNext = false;

  @property({ type: Boolean })
  hideFooter = false;

  private _unsubscribe?: () => void;

  connectedCallback() {
    super.connectedCallback();
    this._unsubscribe = wizardState.subscribe((state) => {
      this._wizardState = state;
    });
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    this._unsubscribe?.();
    this._resizeObserver.disconnect();
  }

  // A keyboard can only scroll what it can focus, and some steps have
  // nothing focusable in them. Only when they overflow, so it isn't an
  // extra tab stop on every step.
  private _observeContent(event: Event) {
    const slot = event.target as HTMLSlotElement;
    this._resizeObserver.disconnect();
    this._resizeObserver.observe(slot.parentElement!);
    for (const element of slot.assignedElements()) {
      this._resizeObserver.observe(element);
    }
  }

  private _checkScrolling() {
    const content = this.renderRoot.querySelector(".content");
    this._scrolls = !!content && content.scrollHeight > content.clientHeight;
  }

  get steps(): WizardStep[] {
    return this._wizardState.steps;
  }

  get currentIndex(): number {
    return this._wizardState.currentStepIndex;
  }

  get isFirstStep(): boolean {
    return wizardState.isFirstStep;
  }

  get isLastStep(): boolean {
    return wizardState.isLastStep;
  }

  render() {
    return html`
      <header class="header">
        <wa-button
          appearance="plain"
          @click=${this._onBack}
          ?disabled=${this.isFirstStep || this.hideBack}
          style=${this.hideBack ? "visibility: hidden" : ""}
        >
          <span slot="start">←</span> ${localize("common.back")}
        </wa-button>

        <div class="header-center">
          <step-indicator
            .steps=${this.steps}
            .currentIndex=${this.currentIndex}
          ></step-indicator>
        </div>

        <div class="header-right"></div>
      </header>

      <main class="content" tabindex=${this._scrolls ? "0" : nothing}>
        <h1 class="flow-title">${this.flowTitle}</h1>
        <slot @slotchange=${this._observeContent}></slot>
      </main>

      ${!this.hideFooter
        ? html`
            <footer class="footer">
              <div class="footer-left">
                <wa-button appearance="plain" @click=${this._onCancel}>
                  ${localize("common.cancel")}
                </wa-button>
              </div>
              <div class="footer-right">
                ${!this.hideNext
                  ? html`
                      <wa-button
                        variant="brand"
                        appearance="accent"
                        @click=${this._onNext}
                        ?disabled=${this.nextDisabled}
                      >
                        ${this.nextLabel}
                      </wa-button>
                    `
                  : ""}
              </div>
            </footer>
          `
        : ""}
    `;
  }

  private _onBack() {
    if (!this.isFirstStep) {
      wizardState.previousStep();
      this.dispatchEvent(
        new CustomEvent("wizard-back", {
          bubbles: true,
          composed: true,
        })
      );
    }
  }

  private _onNext() {
    this.dispatchEvent(
      new CustomEvent("wizard-next", {
        bubbles: true,
        composed: true,
      })
    );
  }

  private _onCancel() {
    this.dispatchEvent(
      new CustomEvent("wizard-cancel", {
        bubbles: true,
        composed: true,
      })
    );
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "wizard-shell": WizardShell;
  }
}
