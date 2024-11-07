## Discord-Keep-Alive

Keeps your discord account online. You can also set a custom status or activity.

## Prerequisites

Make sure the following are installed:

- [node](https://nodejs.org/en/download)
- [pnpm](https://pnpm.io/installation#using-a-standalone-script) (optional)

## Instructions

1. **Clone Project**

   Run the following in a terminal, replace `[repository]` with the git repository link:

   ```
   git clone [repository]
   ```

2. **Install Dependencies**

   `cd` into the cloned directory and run `(p)npm install`.

3. **Configuration**

   Create an `.env` file in the root of the directory with the following variables:

   - `TOKEN` Discord account token. Required.
   - `DEVICE` One of the following:
     - `web` (default)
     - `desktop`
     - `mobile`

   To change the status:

   - `STATUS` One of the following:
     - `online`
     - `idle`
     - `invisible`
     - `dnd`

   To set an activity:

   - `ACTIVITY` Activity name or custom status.
   - `ACTIVITY_TYPE` One of the following:
     - `custom` (custom status)
     - `playing`
     - `streaming`
     - `listening`
     - `watching`
     - `competing`
     - `hang`

   The following variables will only apply if `ACTIVITY_TYPE` is `custom`.

   - `ACTIVITY_EMOJI` Emoji or emoji id.

   The following variables will only apply if `ACTIVITY_TYPE` is **not** `custom`.

   - `ACTIVITY_PLATFORM` One of the following:
     - `desktop`
     - `samsung`
     - `xbox`
     - `ios`
     - `android`
     - `embedded`
     - `ps4`
     - `ps5`
   - `ACTIVITY_TIMESTAMP` Timestamp of when the activity started.
   - `ACTIVITY_APPLICATION_ID` Activity application id, `1` by default.
   - `ACTIVITY_DETAILS` Activity description.
   - `ACTIVITY_URL` Media URL, if streaming.
   - `ACTIVITY_LARGE_IMAGE` Large image, can be a discord cdn url.
   - `ACTIVITY_LARGE_IMAGE_TEXT` Pop-up text when large image is hovered.
   - `ACTIVITY_SMALL_IMAGE` Small image, can be a discord cdn url.
   - `ACTIVITY_SMALL_IMAGE_TEXT` Pop-up text when small image is hovered.

   To add a button to the activity:

   - `ACTIVITY_BUTTON` Button text.
   - `ACTIVITY_BUTTON_URL` Button URL.

   **Tip:** For a second button, use `ACTIVITY_BUTTON_2` and `ACTIVITY_BUTTON_2_URL`.

   To set a "party" for the activity:

   - `ACTIVITY_PARTY_ID` Activity party id, `1` by default.
   - `ACTIVITY_PARTY_CURRENT` Amount of current users.
   - `ACTIVITY_PARTY_MAX` Amount of max users.

4. **Start**

   Run `pnpm start / npm run start`.

## Credits

- [discord.js-selfbot-v13](https://github.com/aiko-chan-ai/discord.js-selfbot-v13), fork of discord.js for selfbots
