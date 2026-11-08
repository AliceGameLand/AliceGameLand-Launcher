/** Keep launch diagnostics useful without copying credentials or personal paths. */
function redactDiagnostic(value, homeDirectory){
    let text = String(value ?? '')
    if(homeDirectory) text = text.split(homeDirectory).join('[user]')
    return text
        .replace(/Bearer\s+[^\s"']+/gi, 'Bearer [redacted]')
        .replace(/((?:access[_-]?token|refresh[_-]?token|id[_-]?token|clientToken|password|authorization)["']?\s*(?:[:=]|\s)\s*)(?:["'][^"'\r\n]*["']|[^\s&,;]+)/gi, '$1[redacted]')
        .replace(/[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]+/g, '[redacted]')
        .replace(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi, '[email]')
        .replace(/[A-Za-z0-9_+/=-]{80,}/g, '[redacted]')
        .slice(0, 2000)
}

function createLaunchDiagnostic({ version, server, stage, error, homeDirectory }){
    return redactDiagnostic([
        `Alice GameLand ${version}`,
        `Time: ${new Date().toISOString()}`,
        `Server: ${server ?? '-'}`,
        `Stage: ${stage}`,
        `Error: ${error?.name ?? 'Error'}${error?.code ? ` (${error.code})` : ''}`,
        error?.message ?? error?.displayable ?? (typeof error === 'string' ? error : stage)
    ].join('\n'), homeDirectory)
}

module.exports = { createLaunchDiagnostic, redactDiagnostic }
