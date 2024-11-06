## Discord-Keep-Alive

Keeps your discord account online. You can also set a custom status or activity.

## Prerequisites

Make sure the following are installed:

- [node](https://nodejs.org/en/download)
- [pnpm](https://pnpm.io/installation#using-a-standalone-script) (optional)

## Instructions

1. **Clone Project**

   Run the following in a terminal, replace `[repository]` with the actual url:

   ```
   git clone [repository]
   ```

2. **Install Dependencies**

   Now, `cd` into the cloned directory and run `(p)npm install`.

3. **Configuration**

   Create an `.env` file in the root of the directory with the following variables:

   - `TOKEN` Set this to your discord account token. Required.
   - `DEVICE` Can be set to any of the following:
     - `web` (default)
     - `desktop`
     - `mobile`

   To change the status:

   - `STATUS` Can be set to any of the following:
     - `online`
     - `idle`
     - `invisible`
     - `dnd`

   To set an activity:

   - `ACTIVITY` Activity name or custom status.
   - `ACTIVITY_TYPE` Can be set to any of the following:
     - `custom` (custom status)
     - `playing`
     - `streaming`
     - `listening`
     - `watching`
     - `competing`
     - `hang`

   The following variables will only apply if `ACTIVITY_TYPE` is `custom`.

   - `ACTIVITY_EMOJI` Can be an emoji or emoji id.

   The following variables will only apply if `ACTIVITY_TYPE` is **not** `custom`.

   - `ACTIVITY_PLATFORM` Can be set to any of the following:
     - `desktop`
     - `samsung`
     - `xbox`
     - `ios`
     - `android`
     - `embedded`
     - `ps4`
     - `ps5`
   - `ACTIVITY_TIMESTAMP` Timestamp of when the activity started.
   - `ACTIVITY_APPLICATION_ID` Activity application id. `1` by default.
   - `ACTIVITY_DETAILS` Activity description.
   - `ACTIVITY_URL` If streaming, URL of media.
   - `ACTIVITY_LARGE_IMAGE` Large image, can be a discord cdn url.
   - `ACTIVITY_LARGE_IMAGE_TEXT` Pop-up text when large image is hovered.
   - `ACTIVITY_SMALL_IMAGE` Small image, can be a discord cdn url.
   - `ACTIVITY_SMALL_IMAGE_TEXT` Pop-up text when small image is hovered.

   To add a button to the activity:

   - `ACTIVITY_BUTTON` Button text.
   - `ACTIVITY_BUTTON_URL` What URL the button points to.

   **Tip:** You can also add a second button via `ACTIVITY_BUTTON_2` and `ACTIVITY_BUTTON_2_URL`.

   To set a "party" for the activity:

   - `ACTIVITY_PARTY_ID` Activity party id. `1` by default.
   - `ACTIVITY_PARTY_CURRENT` Amount of current users.
   - `ACTIVITY_PARTY_MAX` Amount of max users.

4. **Start**

   To start, run `pnpm start / npm run start`.

## Credits

- [discord.js-selfbot-v13](https://github.com/aiko-chan-ai/discord.js-selfbot-v13), fork of discord.js for selfbots
