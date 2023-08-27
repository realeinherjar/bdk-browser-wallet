import { test, expect } from "@playwright/test";

test("api/balance retuns a balance", async ({ request }) => {
  const response = await request.get("/api/balance", {
    headers: {
      Accept: "application/json",
    },
  });

  const body = await response.json();
  console.log(body);

  expect(response.ok()).toBeTruthy();
  expect(response.status()).toBe(200);
  expect(body).toBeGreaterThanOrEqual(0);
});
