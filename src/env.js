import dotenv from "@dotenvx/dotenvx";

dotenv.config({
  path: process.env.ENV_FILES?.split(",") || [".env.dev", ".env"],
  ignore: ["MISSING_ENV_FILE"],
  logLevel: "warn",
  quiet: true,
});
