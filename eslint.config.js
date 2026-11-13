import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';

// Scoped lint: only the React hooks rules — exactly what the
// eslint-disable comments in src/ were written for. No stylistic rules;
// formatting is Prettier-free by design (tsc + tests gate the rest).
export default tseslint.config(
  { ignores: ['dist/**', 'src-tauri/**', 'node_modules/**'] },
  {
    files: ['src/**/*.{ts,tsx}'],
    plugins: { 'react-hooks': reactHooks },
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    rules: {
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
    },
  },
);
