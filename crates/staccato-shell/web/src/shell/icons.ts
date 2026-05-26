import {
  AppWindow,
  Battery,
  Bell,
  Bluetooth,
  Files,
  Gauge,
  Globe,
  Headphones,
  Network,
  Power,
  RefreshCw,
  Search,
  Settings,
  Shield,
  Sparkles,
  Terminal,
  Volume2,
  Wifi,
  X,
  createElement,
  type IconNode,
} from "lucide";

const icons: Record<string, IconNode> = {
  app: AppWindow,
  battery: Battery,
  bell: Bell,
  bluetooth: Bluetooth,
  browser: Globe,
  close: X,
  files: Files,
  gauge: Gauge,
  headphones: Headphones,
  network: Network,
  power: Power,
  reload: RefreshCw,
  search: Search,
  settings: Settings,
  shield: Shield,
  sparkles: Sparkles,
  terminal: Terminal,
  volume: Volume2,
  wifi: Wifi,
};

export const icon = (name: string) =>
  createElement(icons[name] ?? AppWindow, {
    "aria-hidden": "true",
    width: 24,
    height: 24,
    "stroke-width": 1.9,
  }).outerHTML;

export const iconNode = (name: string) => icons[name] ?? AppWindow;
