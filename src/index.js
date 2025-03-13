import "./env.js";

import { Client } from "discord.js-selfbot-v13";
import chalk from "chalk";
import { CustomStatus } from "discord.js-selfbot-v13";
import { RichPresence } from "discord.js-selfbot-v13";

const validDevices = ["web", "desktop", "mobile"];
const validStatuses = ["online", "idle", "invisible", "dnd"];

const validActivityTypes = [
  "custom",
  "playing",
  "streaming",
  "listening",
  "watching",
  "competing",
  "hang",
];
const validActivityPlatforms = [
  "desktop",
  "samsung",
  "xbox",
  "ios",
  "android",
  "embedded",
  "ps4",
  "ps5",
];

const device =
  validDevices.includes(process.env.DEVICE?.toLowerCase()) &&
  process.env.DEVICE.toLowerCase();
const status =
  validStatuses.includes(process.env.STATUS?.toLowerCase()) &&
  process.env.STATUS.toLowerCase();

const activityName = process.env.ACTIVITY;
const activityType =
  validActivityTypes.includes(process.env.ACTIVITY_TYPE?.toLowerCase()) &&
  process.env.ACTIVITY_TYPE.toUpperCase();
const activityEmoji = process.env.ACTIVITY_EMOJI;
const activityPlatform =
  validActivityPlatforms.includes(
    process.env.ACTIVITY_PLATFORM?.toLowerCase(),
  ) && process.env.ACTIVITY_PLATFORM.toLowerCase();
const activityTimestamp = process.env.ACTIVITY_TIMESTAMP;
const activityApplicationId = process.env.ACTIVITY_APPLICATION_ID || "1";
const activityDetails = process.env.ACTIVITY_DETAILS;
const activityUrl = process.env.ACTIVITY_URL;
const activityLargeImage = process.env.ACTIVITY_LARGE_IMAGE;
const activityLargeImageText = process.env.ACTIVITY_LARGE_IMAGE_TEXT;
const activitySmallImage = process.env.ACTIVITY_SMALL_IMAGE;
const activitySmallImageText = process.env.ACTIVITY_SMALL_IMAGE_TEXT;
const activityButton = process.env.ACTIVITY_BUTTON;
const activityButtonURL = process.env.ACTIVITY_BUTTON_URL;
const activityButton2 = process.env.ACTIVITY_BUTTON_2;
const activityButton2URL = process.env.ACTIVITY_BUTTON_2_URL;
const activityPartyId = process.env.ACTIVITY_PARTY_ID || "1";
const activityPartyCurrent = process.env.ACTIVITY_PARTY_CURRENT;
const activityPartyMax = process.env.ACTIVITY_PARTY_MAX;

const token = process.env.TOKEN;
const client = new Client({
  ws: {
    properties: {
      browser:
        device === "desktop"
          ? "Discord Client"
          : device === "mobile"
            ? "Discord iOS"
            : undefined,
    },
  },
});

client.on("ready", function () {
  process.on("SIGINT", async function () {
    client.destroy();
    console.log(chalk.green("Disconnected client."));
    process.exit(0);
  });

  const discriminator =
    client.user.discriminator !== "0" && client.user.discriminator;

  const username = discriminator
    ? `${client.user.username}#${discriminator}`
    : client.user.username;

  console.log(chalk.green(`Logged in as ${username}.`));

  if (device) {
    console.log(chalk.green(`Set device to '${device}'.`));
  }

  if (status) {
    client.user.setStatus(status);
    console.log(chalk.green(`Set status to '${status}'.`));
  }

  if (activityName) {
    let activity;

    if (activityType?.toLowerCase() === "custom") {
      activity = new CustomStatus(client, {
        state: activityName,
        emoji: activityEmoji,
      });

      console.log(chalk.green(`Set custom status to '${activityName}'.`));

      if (activityEmoji) {
        console.log(
          chalk.green(`Set custom status emoji to '${activityEmoji}'.`),
        );
      }
    } else {
      const richPresence = new RichPresence(client).setName(activityName);
      console.log(chalk.green(`Set activity name to '${activityName}'.`));

      if (activityType) {
        richPresence.setType(activityType);
        console.log(chalk.green(`Set activity type to '${activityType}'.`));
      }

      if (activityPlatform) {
        richPresence.setPlatform(activityPlatform);
        console.log(
          chalk.green(`Set activity platform to '${activityPlatform}'.`),
        );
      }

      if (activityTimestamp) {
        try {
          richPresence.setStartTimestamp(parseInt(activityTimestamp));
          console.log(
            chalk.green(`Set activity timestamp to '${activityTimestamp}'.`),
          );
        } catch {
          console.error(
            chalk.red("Failed to parse activity timestamp as number."),
          );
        }
      }

      if (activityApplicationId) {
        richPresence.setApplicationId(activityApplicationId);
        console.log(
          chalk.green(
            `Set activity application id to '${activityApplicationId}'.`,
          ),
        );
      }

      if (activityDetails) {
        richPresence.setDetails(activityDetails);
        console.log(
          chalk.green(`Set activity details to '${activityDetails}'.`),
        );
      }

      if (activityUrl) {
        richPresence.setURL(activityUrl);
        console.log(chalk.green(`Set activity url to '${activityUrl}'.`));
      }

      if (activityLargeImage) {
        richPresence.setAssetsLargeImage(activityLargeImage);
        console.log(
          chalk.green(`Set activity large image to '${activityLargeImage}'.`),
        );
      }

      if (activityLargeImageText) {
        richPresence.setAssetsLargeText(activityLargeImageText);
        console.log(
          chalk.green(
            `Set activity large image text to '${activityLargeImageText}'.`,
          ),
        );
      }

      if (activitySmallImage) {
        richPresence.setAssetsSmallImage(activitySmallImage);
        console.log(
          chalk.green(`Set activity small image to '${activitySmallImage}'.`),
        );
      }

      if (activitySmallImageText) {
        richPresence.setAssetsSmallText(activitySmallImageText);
        console.log(
          chalk.green(
            `Set activity small image text to '${activitySmallImageText}'.`,
          ),
        );
      }

      if (activityButton && activityButtonURL) {
        richPresence.addButton(activityButton, activityButtonURL);
        console.log(
          chalk.green(
            `Set activity button to '${activityButton}' (url: '${activityButtonURL}').`,
          ),
        );
      }

      if (activityButton2 && activityButton2URL) {
        richPresence.addButton(activityButton2, activityButton2URL);
        console.log(
          chalk.green(
            `Set activity button 2 to '${activityButton2}' (url: '${activityButton2URL}').`,
          ),
        );
      }

      if (activityPartyId && activityPartyCurrent && activityPartyMax) {
        try {
          richPresence.setParty({
            id: activityPartyId,
            current: parseInt(activityPartyCurrent),
            max: parseInt(activityPartyMax),
          });

          console.log(
            chalk.green(
              `Set activity party (id: '${activityPartyId}', current: '${activityPartyCurrent}', max: '${activityPartyMax}').`,
            ),
          );
        } catch {
          console.error(
            chalk.red(
              "Failed to parse activity party current or max as number.",
            ),
          );
        }
      }

      activity = richPresence;
    }

    client.user.setActivity(activity);
  }
});

if (token && token !== "") {
  client.login(token);
} else {
  console.error(chalk.red("Token was not provided."));
}
