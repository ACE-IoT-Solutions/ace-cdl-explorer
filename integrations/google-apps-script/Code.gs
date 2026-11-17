/**
 * @OnlyCurrentDoc
 * Adapted from BACnet Studio; access is restricted to the bound workbook.
 */

const QUIZ_RESULTS_SHEET_NAME = 'CDL Quiz Results';
const QUIZ_RESULTS_HEADERS = Object.freeze([
  'Submitted At',
  'Session ID',
  'Email',
  'Score',
  'Total',
  'Percentage',
  'Mastered Total',
  'Started At',
  'Completed At',
  'Bank Version',
  'Question IDs',
  'Answers',
  'Incorrect IDs',
  'Source URL',
  'Pipedrive Person ID',
  'Pipedrive Status',
  'Pipedrive Error',
  'Consent At',
  'Privacy Policy URL',
  'App ID',
  'Question Revisions'
]);

/** Run once from the Apps Script editor while this project is bound to the results spreadsheet. */
function setupQuizResults() {
  const spreadsheet = SpreadsheetApp.getActiveSpreadsheet();
  if (!spreadsheet) throw new Error('Bind this Apps Script project to the Google Sheet before running setup.');
  const properties = PropertiesService.getScriptProperties();
  if (!properties.getProperty('SHEET_NAME')) properties.setProperty('SHEET_NAME', QUIZ_RESULTS_SHEET_NAME);
  ensureResultsSheet_(spreadsheet);
  return `Quiz results will be stored in ${spreadsheet.getName()}.`;
}

function doGet() {
  try {
    getResultsSpreadsheet_();
    return jsonResponse_({ ok: true, service: 'ACE CDL Explorer quiz results', storage: 'bound_workbook' });
  } catch (error) {
    return jsonResponse_({ ok: false, service: 'ACE CDL Explorer quiz results', error: String(error && error.message ? error.message : error) });
  }
}

function doPost(event) {
  try {
    const rawPayload = event && event.parameter && event.parameter.payload;
    if (!rawPayload || rawPayload.length > 50000) throw new Error('Missing or oversized quiz payload.');
    const payload = JSON.parse(rawPayload);
    validateQuizPayload_(payload);

    const stored = appendQuizResult_(payload);
    if (stored.duplicate) return jsonResponse_({ ok: true, duplicate: true, sessionId: payload.sessionId });

    let pipedriveResult;
    try {
      pipedriveResult = syncPipedrive_(payload);
    } catch (error) {
      pipedriveResult = { personId: '', status: 'failed', error: String(error && error.message ? error.message : error) };
    }
    updatePipedriveResult_(stored.sheet, stored.row, pipedriveResult);

    return jsonResponse_({
      ok: true,
      sessionId: payload.sessionId,
      pipedriveStatus: pipedriveResult.status
    });
  } catch (error) {
    console.error(error);
    return jsonResponse_({ ok: false, error: String(error && error.message ? error.message : error) });
  }
}

function validateQuizPayload_(payload) {
  if (!payload || payload.version !== 1 || payload.appId !== 'ace-cdl-explorer') throw new Error('Unsupported quiz result.');
  if (payload.website) throw new Error('Spam check failed.');
  if (typeof payload.email !== 'string' || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(payload.email) || payload.email.length > 254) throw new Error('Invalid email.');
  if (!/^[A-Za-z0-9-]{8,100}$/.test(String(payload.sessionId || ''))) throw new Error('Invalid session ID.');
  if (!Array.isArray(payload.questionIds) || payload.questionIds.length !== 15 || new Set(payload.questionIds).size !== 15 ||
      !payload.questionIds.every(id => /^cdl-\d{3}$/.test(id) && Number(id.slice(4)) >= 1 && Number(id.slice(4)) <= 50)) throw new Error('Expected 15 distinct CDL question IDs.');
  if (!payload.answers || typeof payload.answers !== 'object' || Array.isArray(payload.answers) ||
      Object.keys(payload.answers).length !== 15 || !payload.questionIds.every(id => /^[ABCD]$/.test(payload.answers[id]))) throw new Error('Invalid quiz answers.');
  if (payload.total !== 15 || !Number.isInteger(payload.score) || payload.score < 0 || payload.score > 15) throw new Error('Invalid quiz score.');
  if (!Number.isFinite(payload.percentage) || Math.abs(payload.percentage - payload.score / 15 * 100) > 0.11) throw new Error('Invalid percentage.');
  if (!Number.isInteger(payload.masteredTotal) || payload.masteredTotal < 0 || payload.masteredTotal > 50) throw new Error('Invalid mastery count.');
  if (!Number.isInteger(payload.bankVersion) || payload.bankVersion < 1 || !Array.isArray(payload.questionRevisions) ||
      payload.questionRevisions.length !== 15 || !payload.questionRevisions.every(n => Number.isInteger(n) && n > 0)) throw new Error('Invalid bank revision.');
  if (!Array.isArray(payload.incorrectQuestionIds) || new Set(payload.incorrectQuestionIds).size !== payload.incorrectQuestionIds.length ||
      payload.incorrectQuestionIds.length !== 15 - payload.score || !payload.incorrectQuestionIds.every(id => payload.questionIds.includes(id))) throw new Error('Invalid missed-question list.');
  ['consentAt', 'startedAt', 'completedAt'].forEach(key => {
    if (typeof payload[key] !== 'string' || payload[key].length > 100 || !Number.isFinite(Date.parse(payload[key]))) throw new Error('Invalid ' + key);
  });
  if (Date.parse(payload.startedAt) > Date.parse(payload.completedAt) || Date.parse(payload.consentAt) > Date.parse(payload.completedAt)) throw new Error('Invalid event order.');
  if (payload.privacyPolicyUrl !== 'https://www.privacypolicyonline.com/live.php?token=IuZqxCYZLiM2psa7Bf3mOhJ1tESPxPQU') throw new Error('Missing privacy policy consent.');
  if (typeof payload.sourceUrl !== 'string' || payload.sourceUrl.length > 2000 || !/^https:\/\//.test(payload.sourceUrl)) throw new Error('Invalid source URL.');
}

function appendQuizResult_(payload) {
  const lock = LockService.getScriptLock();
  lock.waitLock(10000);
  try {
    const spreadsheet = getResultsSpreadsheet_();
    const sheet = ensureResultsSheet_(spreadsheet);
    const lastRow = sheet.getLastRow();
    if (lastRow > 1) {
      const existing = sheet.getRange(2, 2, lastRow - 1, 1)
        .createTextFinder(payload.sessionId)
        .matchEntireCell(true)
        .findNext();
      if (existing) return { duplicate: true, sheet, row: existing.getRow() };
    }

    sheet.appendRow([
      new Date(),
      payload.sessionId,
      sheetText_(String(payload.email).toLowerCase()),
      payload.score,
      payload.total,
      payload.percentage,
      payload.masteredTotal,
      payload.startedAt,
      payload.completedAt,
      payload.bankVersion,
      JSON.stringify(payload.questionIds),
      JSON.stringify(payload.answers),
      JSON.stringify(payload.incorrectQuestionIds || []),
      payload.sourceUrl || '',
      '',
      'pending',
      '',
      payload.consentAt,
      payload.privacyPolicyUrl,
      payload.appId,
      JSON.stringify(payload.questionRevisions)
    ]);
    return { duplicate: false, sheet, row: sheet.getLastRow() };
  } finally {
    lock.releaseLock();
  }
}

function getResultsSpreadsheet_() {
  const spreadsheet = SpreadsheetApp.getActiveSpreadsheet();
  if (!spreadsheet) throw new Error('No bound workbook is available. Create this script from the destination Sheet and run setupQuizResults. Broader file access will not be attempted.');
  return spreadsheet;
}

function ensureResultsSheet_(spreadsheet) {
  const sheetName = PropertiesService.getScriptProperties().getProperty('SHEET_NAME') || QUIZ_RESULTS_SHEET_NAME;
  const sheet = spreadsheet.getSheetByName(sheetName) || spreadsheet.insertSheet(sheetName);
  if (sheet.getLastRow() === 0) {
    sheet.getRange(1, 1, 1, QUIZ_RESULTS_HEADERS.length).setValues([QUIZ_RESULTS_HEADERS]);
    sheet.setFrozenRows(1);
    sheet.getRange(1, 1, 1, QUIZ_RESULTS_HEADERS.length).setFontWeight('bold');
  }
  return sheet;
}

function syncPipedrive_(payload) {
  const properties = PropertiesService.getScriptProperties();
  const apiToken = properties.getProperty('PIPEDRIVE_API_TOKEN');
  const companyDomain = normalizePipedriveDomain_(properties.getProperty('PIPEDRIVE_COMPANY_DOMAIN'));
  if (!apiToken || !companyDomain) return { personId: '', status: 'not_configured', error: '' };

  const search = pipedriveRequest_(companyDomain, apiToken, '/api/v2/persons/search', 'get', null, {
    term: payload.email,
    fields: 'email',
    exact_match: 'true',
    limit: '1'
  });
  const firstResult = search.data && search.data.items && search.data.items[0];
  let personId = firstResult && (firstResult.item ? firstResult.item.id : firstResult.id);
  let status = 'matched';

  if (!personId) {
    const created = pipedriveRequest_(companyDomain, apiToken, '/api/v2/persons', 'post', {
      name: payload.email,
      emails: [{ value: payload.email, primary: true, label: 'work' }]
    });
    personId = created.data && created.data.id;
    status = 'created';
  }
  if (!personId) throw new Error('Pipedrive did not return a person ID.');

  const noteContent = [
    '<strong>ACE CDL Explorer competency quiz</strong>',
    `Score: ${Number(payload.score)} / ${Number(payload.total)} (${Number(payload.percentage)}%)`,
    `Completed: ${escapeHtml_(payload.completedAt)}`,
    `Mastered total: ${Number(payload.masteredTotal)}`,
    `Session: ${escapeHtml_(payload.sessionId)}`,
    `Incorrect question IDs: ${escapeHtml_((payload.incorrectQuestionIds || []).join(', ') || 'None')}`
  ].join('<br>');
  pipedriveRequest_(companyDomain, apiToken, '/api/v1/notes', 'post', {
    person_id: Number(personId),
    content: noteContent
  });
  return { personId, status: `${status}_and_noted`, error: '' };
}

function pipedriveRequest_(companyDomain, apiToken, apiPath, method, payload, query) {
  const parameters = Object.assign({}, query || {}, { api_token: apiToken });
  const queryString = Object.keys(parameters)
    .map(key => `${encodeURIComponent(key)}=${encodeURIComponent(parameters[key])}`)
    .join('&');
  const options = {
    method,
    contentType: 'application/json',
    muteHttpExceptions: true
  };
  if (payload) options.payload = JSON.stringify(payload);
  const response = UrlFetchApp.fetch(`https://${companyDomain}.pipedrive.com${apiPath}?${queryString}`, options);
  const statusCode = response.getResponseCode();
  const responseBody = response.getContentText();
  const parsed = responseBody ? JSON.parse(responseBody) : {};
  if (statusCode < 200 || statusCode >= 300 || parsed.success === false) {
    throw new Error(`Pipedrive ${method.toUpperCase()} ${apiPath} failed (${statusCode}): ${responseBody.slice(0, 500)}`);
  }
  return parsed;
}

function normalizePipedriveDomain_(value) {
  return String(value || '')
    .trim()
    .replace(/^https?:\/\//, '')
    .replace(/\.pipedrive\.com.*$/, '')
    .replace(/[^a-zA-Z0-9-]/g, '');
}

function updatePipedriveResult_(sheet, row, result) {
  sheet.getRange(row, 15, 1, 3).setValues([[
    result.personId || '',
    result.status || 'failed',
    sheetText_(result.error || '')
  ]]);
}

function sheetText_(value) {
  const text = String(value);
  return /^[=+\-@]/.test(text) ? "'" + text : text;
}

function escapeHtml_(value) {
  return String(value).replace(/[&<>'"]/g, character => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;'
  })[character]);
}

function jsonResponse_(value) {
  return ContentService.createTextOutput(JSON.stringify(value)).setMimeType(ContentService.MimeType.JSON);
}
