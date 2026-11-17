// Match the source app: result submission is enabled only in canonical CI builds.
export function quizResultsEndpointForBuild(command, environment) {
  const endpoint = environment.QUIZ_RESULTS_ENDPOINT?.trim() ?? ''
  return command === 'build' && environment.CI === 'true' &&
    environment.GITHUB_REPOSITORY === 'ACE-IoT-Solutions/ace-cdl-explorer' &&
    /^https:\/\/script\.google\.com\/macros\/s\/[A-Za-z0-9_-]+\/exec$/.test(endpoint) ? endpoint : ''
}
