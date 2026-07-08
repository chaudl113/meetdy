import i18next from "eslint-plugin-i18next";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "@typescript-eslint/eslint-plugin";
import tsParser from "@typescript-eslint/parser";

export default [
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaFeatures: {
          jsx: true,
        },
      },
    },
    plugins: {
      i18next,
      "react-hooks": reactHooks,
      "@typescript-eslint": tseslint,
    },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
      "@typescript-eslint/no-unused-vars": "warn",
      "prefer-const": "warn",
      "i18next/no-literal-string": [
        "error",
        {
          markupOnly: true, // Only check JSX content, not all strings
          ignoreAttribute: [
            "className",
            "style",
            "type",
            "id",
            "name",
            "key",
            "data-*",
            "aria-*",
          ], // Ignore common non-translatable attributes
        },
      ],
      "sort-imports": ["warn", {
        "ignoreDeclarationSort": false,
        "ignoreCase": true,
      }],
    },
  },
];
