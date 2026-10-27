import { mount } from "svelte";
import App from "./App.svelte";
import "@fontsource-variable/atkinson-hyperlegible-next";
import "./styles.css";

const target = document.getElementById("app");
if (!target) {
  throw new Error("élément #app introuvable");
}

export default mount(App, { target });
