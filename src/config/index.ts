import {
  VALID_DEVICES,
  VALID_STATUSES,
  VALID_ACTIVITY_TYPES,
  VALID_ACTIVITY_PLATFORMS,
} from "./constants";

type Device = (typeof VALID_DEVICES)[number];
type Status = (typeof VALID_STATUSES)[number];
type ActivityType = (typeof VALID_ACTIVITY_TYPES)[number];
type ActivityPlatform = (typeof VALID_ACTIVITY_PLATFORMS)[number];

type ConfigActivityImage = {
  image: string | undefined;
  text: string | undefined;
};

type ConfigActivityButton = {
  name: string | undefined;
  url: string | undefined;
};

type ConfigActivityParty = {
  id: string;
  current: string | undefined;
  max: string | undefined;
};

export type ConfigActivity = {
  name: string | undefined;
  type: ActivityType | undefined;
  emoji: string | undefined;
  platform: ActivityPlatform | undefined;
  timestamp: string | undefined;
  applicationId: string;
  details: string | undefined;
  url: string | undefined;
  largeImage: ConfigActivityImage;
  smallImage: ConfigActivityImage;
  button: ConfigActivityButton;
  button2: ConfigActivityButton;
  party: ConfigActivityParty;
};

type Config = {
  token: string | undefined;
  device: Device | undefined;
  status: Status | undefined;
  activity: ConfigActivity;
};

function getValidValue<T extends readonly string[]>(
  value: string | undefined,
  validValues: T,
): T[number] | undefined {
  return value
    ? (validValues.find(
        (v: string): boolean => v.toLowerCase() === value.toLowerCase(),
      ) as T[number] | undefined)
    : undefined;
}

export const config: Config = {
  token: process.env.TOKEN,
  device: getValidValue(process.env.DEVICE, VALID_DEVICES),
  status: getValidValue(process.env.STATUS, VALID_STATUSES),
  activity: {
    name: process.env.ACTIVITY,
    type: getValidValue(process.env.ACTIVITY_TYPE, VALID_ACTIVITY_TYPES),
    emoji: process.env.ACTIVITY_EMOJI,
    platform: getValidValue(
      process.env.ACTIVITY_PLATFORM,
      VALID_ACTIVITY_PLATFORMS,
    ),
    timestamp: process.env.ACTIVITY_TIMESTAMP,
    applicationId: process.env.ACTIVITY_APPLICATION_ID ?? "1",
    details: process.env.ACTIVITY_DETAILS,
    url: process.env.ACTIVITY_URL,
    largeImage: {
      image: process.env.ACTIVITY_LARGE_IMAGE,
      text: process.env.ACTIVITY_LARGE_IMAGE_TEXT,
    },
    smallImage: {
      image: process.env.ACTIVITY_SMALL_IMAGE,
      text: process.env.ACTIVITY_SMALL_IMAGE_TEXT,
    },
    button: {
      name: process.env.ACTIVITY_BUTTON,
      url: process.env.ACTIVITY_BUTTON_URL,
    },
    button2: {
      name: process.env.ACTIVITY_BUTTON_2,
      url: process.env.ACTIVITY_BUTTON_2_URL,
    },
    party: {
      id: process.env.ACTIVITY_PARTY_ID ?? "1",
      current: process.env.ACTIVITY_PARTY_CURRENT,
      max: process.env.ACTIVITY_PARTY_MAX,
    },
  },
};
