import { Client, CustomStatus, RichPresence } from "discord-sb.js";
import { config } from "./config";
import { DEVICE_MAP } from "./config/constants";
import { logSuccess, logError } from "./utils/logger";
import { buildRichPresence } from "./utils/presence";

const client: Client = new Client({
  ws: {
    properties: {
      browser: config.device ? DEVICE_MAP[config.device] : undefined,
    },
  },
});

client.on("ready", function () {
  process.on("SIGINT", async function () {
    client.destroy();
    logSuccess("Disconnected client.");
    process.exit(0);
  });

  const discriminator: string | undefined =
    client.user?.discriminator !== "0" ? client.user?.discriminator : undefined;

  const username: string | undefined = discriminator
    ? `${client.user?.username}#${discriminator}`
    : client.user?.username;

  if (username) {
    logSuccess(`Logged in as ${username}.`);
  }

  if (config.device) {
    logSuccess(`Set device to '${config.device}'.`);
  }

  if (config.status) {
    client.user?.setStatus(config.status);
    logSuccess(`Set status to '${config.status}'.`);
  }

  if (config.activity.name) {
    let activity: CustomStatus | RichPresence;

    if (config.activity.type === "CUSTOM") {
      activity = new CustomStatus(client, {
        state: config.activity.name,
        emoji: config.activity.emoji,
      });

      logSuccess(`Set custom status to '${config.activity.name}'.`);

      if (config.activity.emoji) {
        logSuccess(`Set custom status emoji to '${config.activity.emoji}'.`);
      }
    } else {
      activity = buildRichPresence(client, config.activity);
    }

    if (client.user) client.user.setActivity(activity);
  }
});

if (config.token && config.token !== "") {
  client.login(config.token);
} else {
  logError("Token was not provided.");
}
