/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        moraine: {
          50: "#f4f7f7",
          100: "#e3ebec",
          200: "#c9d8db",
          300: "#a3bfc4",
          400: "#769ca4",
          500: "#5a818a",
          600: "#4d6b74",
          700: "#435961",
          800: "#3b4b52",
          900: "#344047",
          950: "#222b30",
        },
        ice: {
          50: "#f0f9ff",
          100: "#e0f2fe",
          400: "#38bdf8",
          500: "#0ea5e9",
          600: "#0284c7",
        },
      },
      fontFamily: {
        sans: [
          "Inter",
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "Segoe UI",
          "Roboto",
          "sans-serif",
        ],
        mono: [
          "JetBrains Mono",
          "ui-monospace",
          "SFMono-Regular",
          "Menlo",
          "monospace",
        ],
      },
    },
  },
  plugins: [require("@tailwindcss/typography")],
};
