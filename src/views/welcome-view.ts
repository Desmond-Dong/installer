import { localize } from "../localization/localize.js";
import { LitElement, html, css } from "lit";
import {
  ViewAccessibility,
  reducedMotionStyles,
} from "../utils/view-accessibility.js";
import { logFrontendError } from "../utils/diagnostics.js";
import { customElement, state } from "lit/decorators.js";
import { openExternalLink } from "../utils/external-url.js";
import type WaSelect from "@home-assistant/webawesome/dist/components/select/select.js";
import "@home-assistant/webawesome/dist/components/button/button.js";
import "@home-assistant/webawesome/dist/components/select/select.js";
import "@home-assistant/webawesome/dist/components/option/option.js";
import {
  readStoredLanguage,
  setStoredLanguage,
} from "../localization/initialize.js";
import "../components/diagnostics-actions.js";

@customElement("welcome-view")
export class WelcomeView extends LitElement {
  protected readonly _accessibility = new ViewAccessibility(this);
  @state()
  private _logoClickCount = 0;

  private _clickResetTimer?: number;

  static styles = [
    reducedMotionStyles,
    css`
      /* The content centers in the space above the footer, and the footer
         sits at the bottom. A window too low for both scrolls, footer last. */
      :host {
        display: flex;
        flex-direction: column;
        align-items: center;
        box-sizing: border-box;
        height: 100%;
        padding: 2rem var(--ha-page-gutter, 2rem) 1rem;
        text-align: center;
      }

      @media (max-height: 700px) {
        :host {
          min-height: 0;
          overflow-y: auto;
        }

        :host > * {
          flex-shrink: 0;
        }
      }

      @keyframes soft-pulse {
        0%,
        100% {
          transform: scale(1);
          filter: drop-shadow(0 0 0 rgba(24, 188, 242, 0));
        }
        50% {
          transform: scale(1.02);
          filter: drop-shadow(0 0 15px rgba(24, 188, 242, 0.3));
        }
      }

      .logo-container {
        margin: auto 0 2rem;
        font-size: inherit;
        line-height: 1;
      }

      .logo {
        width: 500px;
        max-width: 100%;
        height: auto;
      }

      .logo-container:hover .logo {
        animation: soft-pulse 3s ease-in-out infinite;
      }

      .intro {
        max-width: 520px;
        color: var(--ha-secondary-text-color, #727272);
        line-height: 1.6;
        margin: 0 0 3rem;
      }

      /* Report a problem, the foundation logo and Learn more, with the logo
         in the middle however wide the two sides are */
      .welcome-footer {
        align-self: stretch;
        margin-top: auto;
        padding-top: 2rem;
        display: grid;
        grid-template-columns: 1fr auto auto 1fr;
        align-items: center;
        gap: 1rem;
        font-size: 0.875rem;
        color: var(--ha-secondary-text-color, #727272);
      }

      .report {
        justify-self: start;
        --diagnostics-link-color: var(--ha-secondary-text-color, #727272);
      }

      /* [China mirror] The manual language choice, styled as footer text. */
      .language::part(form-control-label) {
        display: none;
      }

      .language::part(combobox) {
        border: none;
        box-shadow: none;
        background: none;
        font: inherit;
        color: inherit;
        width: max-content;
      }

      .learn-more {
        justify-self: end;
        color: inherit;
        text-decoration: underline;
      }

      /* Too narrow for three columns: stack them around the logo */
      @media (max-width: 600px) {
        .welcome-footer {
          grid-template-columns: auto;
          justify-items: center;
          gap: 0.5rem;
        }

        .report,
        .language,
        .learn-more {
          justify-self: center;
        }

        .ohf-link {
          order: -1;
        }
      }

      .ohf-link {
        display: block;
        line-height: 0;
        text-decoration: none;
      }

      /* Quiet until pointed at: the footer should not compete with Let's go */
      .ohf-logo {
        width: 140px;
        filter: grayscale(1);
        opacity: 0.6;
        transition:
          opacity 0.2s ease,
          filter 0.2s ease;
      }

      .ohf-link:hover .ohf-logo,
      .ohf-link:focus-visible .ohf-logo {
        filter: none;
        opacity: 1;
      }

      /* Both logos come as artwork for each theme */
      .logo-dark,
      .ohf-logo-dark {
        display: none;
      }

      @media (prefers-color-scheme: dark) {
        .logo-light,
        .ohf-logo-light {
          display: none;
        }
        .logo-dark,
        .ohf-logo-dark {
          display: block;
        }
      }
    `,
  ];

  render() {
    return html`
      <h1 class="logo-container" @click=${this._onLogoClick}>
        <img
          class="logo logo-light"
          src="/assets/home-assistant-logo-light.svg"
          alt=${localize("brand.home_assistant")}
        />
        <img
          class="logo logo-dark"
          src="/assets/home-assistant-logo-dark.svg"
          alt=${localize("brand.home_assistant")}
        />
      </h1>

      <p class="intro">${localize("views.welcome_view.subtitle")}</p>

      <wa-button
        variant="brand"
        appearance="accent"
        size="l"
        @click=${this._onLetsGo}
      >
        ${localize("views.welcome_view.let_s_go")} <span slot="end">→</span>
      </wa-button>

      <footer class="welcome-footer">
        <diagnostics-actions class="report"></diagnostics-actions>
        <a
          class="ohf-link"
          href="https://www.openhomefoundation.org/"
          target="_blank"
          rel="noopener noreferrer"
          @click=${openExternalLink}
        >
          <img
            class="ohf-logo ohf-logo-light"
            src="/assets/ohf-logo-light.svg"
            alt=${localize("views.welcome_view.open_home_foundation")}
          />
          <img
            class="ohf-logo ohf-logo-dark"
            src="/assets/ohf-logo-dark.svg"
            alt=${localize("views.welcome_view.open_home_foundation")}
          />
        </a>
        <wa-select
          class="language"
          size="s"
          label=${localize("views.welcome_view.language")}
          .value=${readStoredLanguage() ?? "auto"}
          @change=${this._onLanguageChange}
        >
          <wa-option value="auto"
            >${localize("views.welcome_view.language_auto")}</wa-option
          >
          <wa-option value="en">English</wa-option>
          <wa-option value="zh-CN">中文（简体）</wa-option>
        </wa-select>
        <a
          class="learn-more"
          href="https://www.home-assistant.io/installation/"
          target="_blank"
          rel="noopener noreferrer"
          @click=${openExternalLink}
        >
          ${localize("views.welcome_view.learn_more")}
        </a>
      </footer>
    `;
  }

  private _onLogoClick() {
    this._logoClickCount++;

    // Reset the counter after 2 seconds of no clicks
    if (this._clickResetTimer) {
      clearTimeout(this._clickResetTimer);
    }
    this._clickResetTimer = window.setTimeout(() => {
      this._logoClickCount = 0;
    }, 2000);

    // Play sound after 5 clicks
    if (this._logoClickCount === 5) {
      this._playEasterEgg();
      this._logoClickCount = 0;
    }
  }

  private _playEasterEgg() {
    const audio = new Audio("/assets/audio/home-assistant.mp3");
    audio.play().catch((error) => {
      logFrontendError(error);
    });
  }

  private _onLetsGo() {
    this.dispatchEvent(
      new CustomEvent("navigate", {
        detail: { view: "path-selection" },
        bubbles: true,
        composed: true,
      })
    );
  }

  /** [China mirror] Switch the UI language and restart with it applied. */
  private _onLanguageChange(event: Event) {
    const select = event.target as WaSelect;
    // Multi-selects report string arrays; this one only picks a single tag.
    setStoredLanguage(
      typeof select.value === "string" && select.value !== "auto"
        ? select.value
        : null
    );
    location.reload();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    if (this._clickResetTimer) {
      clearTimeout(this._clickResetTimer);
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "welcome-view": WelcomeView;
  }
}
