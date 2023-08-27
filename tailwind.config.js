module.exports = {
  content: {
    files: ["*.html", "./app/src/**/*.rs", "./node_modules/preline/dist/*.js"],
  },
  theme: {
    extend: {},
  },
  plugins: [require("preline/plugin")],
  darkMode: "class",
};
