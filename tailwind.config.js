/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{vue,js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: "var(--ink)",
        muted: "var(--ink-muted)",
        accent: "var(--accent)",
        panel: "var(--bg-panel)",
        p2p: "var(--p2p)",
        derp: "var(--derp)",
      },
      fontFamily: {
        display: ["var(--font-display)"],
        body: ["var(--font-body)"],
      },
    },
  },
  plugins: [],
};
