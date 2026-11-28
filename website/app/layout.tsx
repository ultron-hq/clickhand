import type { Metadata, Viewport } from "next";
import { Doto, Funnel_Sans, Martian_Mono } from "next/font/google";

// Doto: a dot-matrix display face with square dots, the same points the hero mask is made of.
const display = Doto({ subsets: ["latin"], variable: "--font-display", axes: ["ROND"] });
const text = Funnel_Sans({ subsets: ["latin"], variable: "--font-text" });
const mono = Martian_Mono({ subsets: ["latin"], variable: "--font-mono", axes: ["wdth"] });
import "./globals.css";
import { HudFx } from "@/components/HudFx";

export const metadata: Metadata = {
  metadataBase: new URL("https://openhud.vercel.app"),
  title: "OpenHUD: let any AI agent use your PC like a human",
  description:
    "OpenHUD is an open-source desktop agent for Windows, macOS and Linux. Press Ctrl+Space and ask, or give Claude Code, Codex and any MCP agent the mouse and keyboard.",
  icons: { icon: "/logo.svg" },
  openGraph: {
    title: "OpenHUD",
    description: "An open-source desktop agent. Press Ctrl+Space and ask, or give your AI agent the mouse.",
    images: ["/logo.png"],
  },
};

export const viewport: Viewport = { themeColor: "#0b0d14", colorScheme: "dark" };

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${display.variable} ${text.variable} ${mono.variable}`}>
      <body>
        {children}
        <HudFx />
      </body>
    </html>
  );
}
