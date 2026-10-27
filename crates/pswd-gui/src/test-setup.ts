import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { hideToast } from "./lib/toast.svelte";

// La bulle de confirmation est partagée par toute l'application : elle repart vide à chaque test.
afterEach(() => {
  hideToast();
});
