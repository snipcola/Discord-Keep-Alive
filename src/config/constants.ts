export const DEVICE_MAP = {
  web: undefined,
  desktop: "Discord Client",
  mobile: "Discord iOS",
} as const;

export const VALID_DEVICES = Object.keys(DEVICE_MAP) as Array<
  keyof typeof DEVICE_MAP
>;

export const VALID_STATUSES = ["online", "idle", "invisible", "dnd"] as const;

export const VALID_ACTIVITY_TYPES = [
  "CUSTOM",
  "PLAYING",
  "STREAMING",
  "LISTENING",
  "WATCHING",
  "COMPETING",
  "HANG",
] as const;

export const VALID_ACTIVITY_PLATFORMS = [
  "desktop",
  "samsung",
  "xbox",
  "ios",
  "android",
  "embedded",
  "ps4",
  "ps5",
] as const;
