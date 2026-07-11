import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Cesium ships its assets as static files; the app serves them from a base
// path set at runtime. We keep the config minimal and let Cesium resolve its
// own worker and asset URLs.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "node",
  },
});
