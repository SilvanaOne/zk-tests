/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: "class",
  content: [
    "./src/pages/**/*.{ts,tsx}",
    "./src/components/**/*.{ts,tsx}",
    "./src/app/**/*.{ts,tsx}",
    "./src/**/*.{ts,tsx}",
    "*.{js,ts,jsx,tsx,mdx}",
  ],
  prefix: "",
  theme: {
    container: {
      center: true,
      padding: "2rem",
      screens: {
        "2xl": "1400px",
      },
    },
    extend: {
      fontFamily: {
        sans: ["Inter", "var(--font-inter)", "sans-serif"],
      },
      fontSize: {
        xs: ["0.75rem", "1.1"],
        sm: ["0.875rem", "1.25"],
        base: ["1rem", "1.5"],
        lg: ["1.125rem", "1.6"],
        xl: ["1.25rem", "1.6"],
        "2xl": ["1.5rem", "1.3"],
      },
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        // Silvana Brand Palette
        brand: {
          pink: "var(--brand-pink)",
          purple: "var(--brand-purple)",
          blue: "var(--brand-blue)",
          green: "var(--brand-green)",
          yellow: "var(--brand-yellow)",
          neutral: {
            900: "var(--neutral-900)",
            600: "var(--neutral-600)",
            200: "var(--neutral-200)",
            50: "var(--neutral-50)",
          },
        },
        neutral: {
          900: "var(--neutral-900)",
          600: "var(--neutral-600)",
          50: "var(--neutral-50)",
        },
        // Legacy colors for compatibility
        success: "var(--brand-green)",
        danger: "#FF6F6F",
        surface: "rgba(255,255,255,0.03)",
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
        "fade-in-up": {
          from: { opacity: "0", transform: "translateY(20px)" },
          to: { opacity: "1", transform: "translateY(0)" },
        },
        "pulse-success": {
          "0%, 100%": { backgroundColor: "var(--success)" },
          "50%": { backgroundColor: "rgba(42, 232, 158, 0.7)" },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
        "fade-in-up": "fade-in-up 0.4s ease-out",
        "pulse-success": "pulse-success 0.6s ease-in-out",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
};
