import "./styles/main.css";
import App from "./App.svelte";
import { mount } from "svelte";

const target = document.querySelector<HTMLElement>("#app");

if (!target) {
  throw new Error("missing shell root");
}

mount(App, { target });
