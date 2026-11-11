import type { Config } from 'tailwindcss';

export default {
  darkMode: 'class',
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        background: '#000000',
        surface: '#1C1C1E',
        elevated: '#2C2C2E',
        border: '#38383A',
        foreground: '#F5F5F7',
        muted: {
          DEFAULT: '#98989D',
          foreground: '#636366',
        },
        accent: {
          DEFAULT: '#0A84FF',
          hover: '#409CFF',
        },
        success: '#30D158',
        warning: '#FF9F0A',
        danger: '#FF453A',
      },
      fontFamily: {
        sans: ['Inter', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'sans-serif'],
        mono: ['JetBrains Mono', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'Consolas', 'monospace'],
      },
      fontSize: {
        display: ['48px', { lineHeight: '1.2', letterSpacing: '-0.02em', fontWeight: '700' }],
        'title-1': ['32px', { lineHeight: '1.2', letterSpacing: '-0.01em', fontWeight: '600' }],
        'title-2': ['24px', { lineHeight: '1.2', fontWeight: '600' }],
        'title-3': ['20px', { lineHeight: '1.2', fontWeight: '600' }],
        body: ['16px', { lineHeight: '1.5', fontWeight: '400' }],
        'body-sm': ['14px', { lineHeight: '1.5', fontWeight: '400' }],
        caption: ['12px', { lineHeight: '1.4', letterSpacing: '0.01em', fontWeight: '500' }],
        mono: ['13px', { lineHeight: '1.4', fontWeight: '500' }],
      },
      borderRadius: {
        input: '10px',
        button: '10px',
        card: '16px',
        dialog: '20px',
      },
      spacing: {
        1: '4px',
        2: '8px',
        3: '12px',
        4: '16px',
        6: '24px',
        8: '32px',
        12: '48px',
        16: '64px',
        24: '96px',
      },
      boxShadow: {
        modal: '0 20px 40px rgba(0, 0, 0, 0.6)',
        focus: '0 0 0 3px rgba(10, 132, 255, 0.3)',
      },
    },
  },
  plugins: [],
} satisfies Config;
