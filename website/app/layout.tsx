import type { Metadata, Viewport } from "next";
import { GeistSans } from "geist/font/sans";
import { GeistMono } from "geist/font/mono";
import "./globals.css";

export const metadata: Metadata = {
  metadataBase: new URL("https://getultron.vercel.app"),
  title: "Ultron: let any AI agent use your PC like a human",
  description:
    "Ultron is an open-source desktop agent for Windows, macOS and Linux. Press Ctrl+Space and ask, or give Claude Code, Codex and any MCP agent the mouse and keyboard.",
  icons: { icon: "/logo.svg" },
  openGraph: {
    title: "Ultron",
    description: "An open-source desktop agent. Press Ctrl+Space and ask, or give your AI agent the mouse.",
    images: ["/logo.png"],
  },
};

export const viewport: Viewport = { themeColor: "#0b0d14", colorScheme: "dark" };

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${GeistSans.variable} ${GeistMono.variable}`}>
      <body>{children}</body>
    </html>
  );
}
