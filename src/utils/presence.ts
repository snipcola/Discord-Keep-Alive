import { Client, RichPresence } from "discord-sb.js";
import type { ConfigActivity } from "../config";
import { logSuccess, logError } from "./logger";

export function buildRichPresence(
  client: Client,
  activity: ConfigActivity,
): RichPresence {
  const presence: RichPresence = new RichPresence(client).setName(
    activity.name,
  );

  logSuccess(`Set activity name to '${activity.name}'.`);

  if (activity.type) {
    presence.setType(activity.type);
    logSuccess(`Set activity type to '${activity.type}'.`);
  }

  if (activity.platform) {
    presence.setPlatform(activity.platform);
    logSuccess(`Set activity platform to '${activity.platform}'.`);
  }

  if (activity.timestamp) {
    try {
      presence.setStartTimestamp(parseInt(activity.timestamp));
      logSuccess(`Set activity timestamp to '${activity.timestamp}'.`);
    } catch {
      logError("Failed to parse activity timestamp as number.");
    }
  }

  if (activity.applicationId) {
    presence.setApplicationId(activity.applicationId);
    logSuccess(`Set activity application id to '${activity.applicationId}'.`);
  }

  if (activity.details) {
    presence.setDetails(activity.details);
    logSuccess(`Set activity details to '${activity.details}'.`);
  }

  if (activity.url) {
    presence.setURL(activity.url);
    logSuccess(`Set activity url to '${activity.url}'.`);
  }

  if (activity.largeImage.image) {
    presence.setAssetsLargeImage(activity.largeImage.image);
    logSuccess(`Set activity large image to '${activity.largeImage.image}'.`);
  }

  if (activity.largeImage.text) {
    presence.setAssetsLargeText(activity.largeImage.text);
    logSuccess(
      `Set activity large image text to '${activity.largeImage.text}'.`,
    );
  }

  if (activity.smallImage.image) {
    presence.setAssetsSmallImage(activity.smallImage.image);
    logSuccess(`Set activity small image to '${activity.smallImage.image}'.`);
  }

  if (activity.smallImage.text) {
    presence.setAssetsSmallText(activity.smallImage.text);
    logSuccess(
      `Set activity small image text to '${activity.smallImage.text}'.`,
    );
  }

  if (activity.button.name && activity.button.url) {
    presence.addButton(activity.button.name, activity.button.url);

    logSuccess(
      `Set activity button to '${activity.button.name}' (url: '${activity.button.url}').`,
    );
  }

  if (activity.button2.name && activity.button2.url) {
    presence.addButton(activity.button2.name, activity.button2.url);

    logSuccess(
      `Set activity button 2 to '${activity.button2.name}' (url: '${activity.button2.url}').`,
    );
  }

  if (activity.party.id && activity.party.current && activity.party.max) {
    try {
      presence.setParty({
        id: activity.party.id,
        current: parseInt(activity.party.current),
        max: parseInt(activity.party.max),
      });

      logSuccess(
        `Set activity party (id: '${activity.party.id}', current: '${activity.party.current}', max: '${activity.party.max}').`,
      );
    } catch {
      logError("Failed to parse activity party current or max as number.");
    }
  }

  return presence;
}
