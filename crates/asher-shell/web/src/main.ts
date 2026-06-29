import "./styles/main.css";
import App from "./App.svelte";
import { getSnapshot } from "./shell/bridge";
import { mount } from "svelte";

const target = document.querySelector<HTMLElement>("#app");

if (!target) {
  throw new Error("missing shell root");
}

const initialSnapshot = getSnapshot();
target.dataset.animations = initialSnapshot.appearance.animationsEnabled ? "true" : "false";

mount(App, { target });
