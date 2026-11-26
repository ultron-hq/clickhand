import path from "node:path";
import type { NextConfig } from "next";

const config: NextConfig = {
  reactStrictMode: true,
  turbopack: { root: path.join(__dirname) },
};

export default config;
