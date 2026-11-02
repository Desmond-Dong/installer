import { test, expect } from "./fixtures.js";

/** Runs the app as the desktop build does, with an English system locale. */
async function startOnAnEnglishSystem(page: import("@playwright/test").Page) {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "languages", { value: ["en-GB"] });
    Object.assign(window, {
      isTauri: true,
      __TAURI_INTERNALS__: {
        transformCallback: () => 1,
        invoke: async (command: string) => {
          if (command === "plugin:event|listen") return 1;
          if (command === "plugin:os|locale") return "en-US";
          throw new Error(`Unexpected command: ${command}`);
        },
      },
    });
  });
}

test("a chosen language outranks the system locale", async ({ page }) => {
  // [China mirror] The choice must survive the system locale, which detection
  // would otherwise read first. This is what silently broke the select.
  await startOnAnEnglishSystem(page);
  await page.addInitScript(() => {
    localStorage.setItem("hai.language", "zh-CN");
  });
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
  await expect(page.getByRole("button", { name: "开始吧" })).toBeVisible();
});

test("the language select switches the whole interface", async ({ page }) => {
  await startOnAnEnglishSystem(page);
  await page.goto("/");
  await expect(page.getByRole("button", { name: "Let's go" })).toBeVisible();

  // Picking a language stores it and restarts, as the select does by hand
  await page.evaluate(() => {
    const view = document
      .querySelector("app-shell")!
      .shadowRoot!.querySelector("welcome-view")!;
    const select = view.shadowRoot!.querySelector("wa-select.language") as
      | (HTMLElement & { value: string })
      | null;
    if (!select) throw new Error("the language select is not on screen");
    select.value = "zh-CN";
    select.dispatchEvent(new Event("change", { bubbles: true }));
  });

  await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
  await expect(page.getByRole("button", { name: "开始吧" })).toBeVisible();
  await expect(page).toHaveTitle("Home Assistant 安装器");
});

for (const mode of ["native", "invalid", "rejected", "stalled"] as const) {
  test(`locale discovery ${mode} still starts the English UI`, async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.addInitScript((mode) => {
      Object.defineProperty(navigator, "languages", { value: ["en-GB"] });
      Object.assign(window, {
        isTauri: true,
        __TAURI_INTERNALS__: {
          transformCallback: () => 1,
          invoke: async (command: string) => {
            // The app shell listens for blocked closes on startup
            if (command === "plugin:event|listen") return 1;
            if (command !== "plugin:os|locale")
              throw new Error(`Unexpected command: ${command}`);
            if (mode === "rejected") throw new Error("Locale unavailable");
            if (mode === "stalled") return new Promise(() => undefined);
            // This delay proves the app waits before importing static wizard labels.
            await new Promise((resolve) => setTimeout(resolve, 100));
            Object.assign(window, {
              localeComponentsLoadedEarly: !!customElements.get("app-shell"),
            });
            return mode === "invalid" ? "invalid_locale" : "en-IN";
          },
        },
      });
    }, mode);
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Let's go" })).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute(
      "lang",
      mode === "native" ? "en-IN" : "en-GB"
    );
    await expect(page).toHaveTitle("Home Assistant Installer");
    if (mode === "native" || mode === "invalid") {
      expect(
        await page.evaluate(() =>
          Reflect.get(window, "localeComponentsLoadedEarly")
        )
      ).toBe(false);
    }
    await page.getByRole("button", { name: "Let's go" }).click();
    await expect(
      page.locator('option-card[heading="Proxmox server"]')
    ).toBeVisible();
    expect(errors).toEqual([]);
  });
}
