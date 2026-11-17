# Quiz results: Google Sheets and Pipedrive

Adapted from BACnet Studio’s learning-record integration.

This bound Google Apps Script receives completed ACE CDL Explorer quizzes, writes each unique session to a Google Sheet, finds or creates a Pipedrive person by exact email, and attaches the result as a Pipedrive note. Sheet storage succeeds even if Pipedrive is unavailable.

## 1. Add the script to the results sheet

1. Open the Google Sheet that will hold quiz results.
2. Select **Extensions → Apps Script**.
3. Replace the editor's `Code.gs` with [`Code.gs`](Code.gs).
4. In **Project Settings**, enable **Show `appsscript.json` manifest file in editor** and replace it with [`appsscript.json`](appsscript.json).
5. Save the project.
6. Select `setupQuizResults` in the function menu, click **Run**, and approve the requested spreadsheet and external-request permissions.

The setup function creates a `CDL Quiz Results` tab and saves the bound workbook's
ID as `SPREADSHEET_ID`. Web requests open that configured ID explicitly: Google's
[bound-script helpers](https://developers.google.com/apps-script/guides/bound)
are unavailable during web-app execution. The manifest requests the full
`spreadsheets` scope required by
[openById](https://developers.google.com/apps-script/reference/spreadsheet/spreadsheet-app#openById(String))
and external requests for optional Pipedrive sync. The code targets the configured
workbook; the Google authorization scope itself is broader than that workbook.
Changing the bound workbook requires rerunning setup before redeployment.

## 2. Configure Pipedrive without exposing its token

In Apps Script, open **Project Settings → Script Properties** and add:

| Property | Value |
|---|---|
| `PIPEDRIVE_COMPANY_DOMAIN` | Your Pipedrive company subdomain, such as `acme` for `acme.pipedrive.com` |
| `PIPEDRIVE_API_TOKEN` | The API token from your Pipedrive personal preferences |

Do not put either value in frontend code. If these properties are absent, quiz results still reach Sheets and the Pipedrive Status column reads `not_configured`.

## 3. Deploy the receiver

1. Select **Deploy → New deployment**.
2. Choose **Web app**.
3. Set **Execute as** to **Me**.
4. Set **Who has access** to **Anyone**.
5. Deploy a versioned release and copy the URL ending in `/exec`.

The endpoint is public by necessity. The handler validates payload shape, rejects a honeypot value, limits payload size, and deduplicates session IDs. Quiz scores are low-stakes client results and should not be treated as proctored certification records.

## 4. Add the endpoint to GitHub Actions

Run this locally, replacing the placeholder with the deployed `/exec` URL:

```bash
gh secret set QUIZ_RESULTS_ENDPOINT \
  --repo ACE-IoT-Solutions/ace-cdl-explorer \
  --body 'https://script.google.com/macros/s/YOUR_DEPLOYMENT_ID/exec'
```

The Vite build injects this endpoint only when GitHub Actions builds the canonical repository. Local builds and forks retain local quiz progress but do not transmit results.

## Updating the Apps Script

After changing `Code.gs`, create a new Apps Script version and edit the existing deployment to use it. Retaining the same deployment keeps the `/exec` URL and GitHub secret unchanged.

## CDL payload and deployment scope

This receiver is for CDL's 15-question rounds and 50-question bank, with letter
answer IDs, question revisions and `appId = ace-cdl-explorer`. It is not a drop-in
replacement for the BACnet 35-question receiver. The default destination tab is
`CDL Quiz Results`; set `SHEET_NAME` in Script Properties to use another tab in
the bound workbook. A dedicated bound deployment can also target a separate tab
in a shared workbook. The workbook/deployment has not been configured by this change.

The sheet records consent time, privacy policy URL, app identity and question
revisions as well as the score and answers. Session-ID deduplication makes a retry
safe. User text is escaped before writing to a spreadsheet cell. Pipedrive failures
are recorded after the Sheet row is committed. Reposting an existing session does
not create another Pipedrive note or retry a previously failed Pipedrive sync.

The browser submits a form-encoded request with `mode: no-cors`, matching the source
app. Its response is opaque: “submitted for processing” means the request was
sent, not that Sheets or Pipedrive acknowledged success. Verify the Sheet after a
real deployment. Local and fork builds have no receiver URL and send no result.
No live submission is needed for the automated tests; they use mocks.

Participants who consent while result collection is disabled are prompted to
consent again if collection is later enabled on the same origin. Changing to a
different participant starts a fresh round; existing round identity is not
rewritten. Browser reset of learning history retains the separately saved email;
use **Change email** to forget the saved participant.
