import {
  captureRenderSignal,
  expect,
  expectInteractionChangesFrame,
  expectRendered,
  openDemo,
  test,
} from "./fixtures.mjs";

test("globe mode loads and renders", async ({ page }, testInfo) => {
  await openDemo(page, "/?no_anise=1");

  const signal = await captureRenderSignal(page, testInfo, "globe-load");
  expectRendered(signal, "globe load");
});

test("globe mode responds to wheel zoom", async ({ page }, testInfo) => {
  await openDemo(page, "/?no_anise=1");
  await page.mouse.move(512, 384);

  await expectInteractionChangesFrame(page, testInfo, "globe-wheel", async () => {
    await page.mouse.wheel(0, -900);
  });
});

test("map mode responds to wheel zoom", async ({ page }, testInfo) => {
  await openDemo(page, "/?view=map&no_anise=1");
  await page.mouse.move(512, 384);

  await expectInteractionChangesFrame(page, testInfo, "map-wheel", async () => {
    await page.mouse.wheel(0, -600);
  });
});

