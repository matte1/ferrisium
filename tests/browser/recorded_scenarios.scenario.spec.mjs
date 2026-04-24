import { expect, gestures, readTestBridge, recordScenario, test } from "./fixtures.mjs";

test("records globe H3 inspection orbit and zoom @scenario", async ({ page }, testInfo) => {
  const { after, before } = await recordScenario(page, testInfo, {
    expectedMode: "globe",
    label: "globe-h3-inspection",
    path: "/?no_anise=1&h3_inspect=1&globe_lon=-96&globe_lat=39&globe_distance_factor=1.45",
    steps: async () => {
      await gestures.dragCanvas(page, {
        from: { x: 0.45, y: 0.48 },
        steps: 24,
        to: { x: 0.66, y: 0.60 },
      });
      await gestures.wheelCanvas(page, -500);
    },
  });

  expect(after.pageMetrics.testBridge.globe.yawDeg).not.toBe(before.pageMetrics.testBridge.globe.yawDeg);
});

test("records map pan and zoom @scenario", async ({ page }, testInfo) => {
  const { after, before } = await recordScenario(page, testInfo, {
    expectedMode: "map",
    label: "map-pan-zoom",
    path: "/?view=map&no_anise=1",
    steps: async () => {
      await gestures.dragCanvas(page, {
        from: { x: 0.55, y: 0.52 },
        steps: 24,
        to: { x: 0.35, y: 0.42 },
      });
      await gestures.wheelCanvas(page, -500, { at: { x: 0.52, y: 0.50 } });
    },
  });

  expect(after.pageMetrics.testBridge.map.zoom).not.toBe(before.pageMetrics.testBridge.map.zoom);
});

test("records solar orbit camera interaction @scenario", async ({ page }, testInfo) => {
  const { after, before } = await recordScenario(page, testInfo, {
    expectedMode: "solar",
    label: "solar-orbit-zoom",
    path: "/?view=solar&no_anise=1&solar_focus=earth&trail_months=1",
    settleMs: 1600,
    steps: async () => {
      await gestures.dragCanvas(page, {
        from: { x: 0.48, y: 0.50 },
        steps: 24,
        to: { x: 0.62, y: 0.40 },
      });
      await gestures.wheelCanvas(page, -450);
    },
  });

  const bridge = await readTestBridge(page);
  expect(bridge.metricCamera.distanceUnits).not.toBe(before.pageMetrics.testBridge.metricCamera.distanceUnits);
  expect(after.pageMetrics.testBridge.metricCamera.yawDeg).not.toBe(
    before.pageMetrics.testBridge.metricCamera.yawDeg,
  );
});
