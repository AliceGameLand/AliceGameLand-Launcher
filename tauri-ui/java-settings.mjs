export function parseOptions(value) {
    const input = value.trim()
    const options = []
    let end = 0
    for (const match of input.matchAll(/(?:[^\s"]+|"(?:[^"\\]|\\.)*")+/g)) {
        if (input.slice(end, match.index).trim()) throw new Error('Java 옵션의 큰따옴표를 확인해 주세요.')
        options.push(match[0].replace(/"((?:[^"\\]|\\.)*)"/g, (_, word) => word.replace(/\\(["\\])/g, '$1')))
        end = match.index + match[0].length
    }
    if (input.slice(end).trim()) throw new Error('Java 옵션의 큰따옴표를 확인해 주세요.')
    return options
}

function gibibytes(value, fallback) {
    const match = /^(\d+)([mg])$/i.exec(value)
    return match ? Number(match[1]) / (match[2].toLowerCase() === 'm' ? 1024 : 1) : fallback
}

export function createJavaSettings(document) {
    const byId = id => document.getElementById(id)
    const fields = ['Min', 'Max'].map(name => ({
        slider: byId('settings' + name + 'RAMRange'),
        label: byId('settings' + name + 'RAMLabel')
    }))
    const argumentsInput = byId('settingsJVMOptsVal')
    let rangeChanged = false
    let originalArguments
    let originalOptions
    let activeServerId
    let upperLimit = 8
    let minimum = 2
    const step = 0.5

    function setValue(index, value) {
        const { slider, label } = fields[index]
        const position = (value - minimum) / (upperLimit - minimum) * 100 + '%'
        slider.setAttribute('aria-valuenow', value)
        slider.setAttribute('aria-valuetext', value + ' GB')
        slider.querySelector('.rangeSliderBar').style.width = position
        slider.querySelector('.rangeSliderTrack').style.left = position
        label.textContent = value + 'G'
    }

    const getValue = index => Number(fields[index].slider.getAttribute('aria-valuenow'))
    function adjust(index, next) {
        rangeChanged = true
        const value = Math.max(minimum, Math.min(upperLimit, next))
        setValue(index, value)
        if (getValue(0) > getValue(1)) setValue(1 - index, value)
    }

    for (const [index, { slider }] of fields.entries()) {
        const fromPointer = event => {
            const bounds = slider.getBoundingClientRect()
            const fraction = (event.clientX - bounds.left) / bounds.width
            adjust(index, minimum + Math.round(fraction * (upperLimit - minimum) / step) * step)
        }
        slider.addEventListener('pointerdown', event => {
            if (event.button !== 0) return
            slider.focus()
            slider.setPointerCapture(event.pointerId)
            fromPointer(event)
        })
        slider.addEventListener('pointermove', event => {
            if (slider.hasPointerCapture(event.pointerId)) fromPointer(event)
        })
        slider.addEventListener('keydown', event => {
            const offsets = { ArrowLeft: -step, ArrowDown: -step, ArrowRight: step, ArrowUp: step }
            let next
            if (event.key === 'Home') next = minimum
            else if (event.key === 'End') next = upperLimit
            else if (Object.hasOwn(offsets, event.key)) next = getValue(index) + offsets[event.key]
            else return
            event.preventDefault()
            adjust(index, next)
        })
    }

    const panel = byId('settingsTabJava')
    byId('settingsJavaExecSel').disabled = true


    return {
        load(server, snapshot) {
            activeServerId = server.id
            panel.querySelector('.settingsSelServContent').textContent = server.name
            byId('settingsJavaReqDesc').textContent = 'Java 25'
            byId('settingsJavaExecDetails').textContent = snapshot.executable
                ? 'Temurin Java 25'
                : '첫 게임 실행 때 Java 25를 내려받아요.'
            byId('settingsJavaExecVal').value = snapshot.executable ?? ''
            byId('settingsJvmOptsLink').hidden = true
            const giga = 1024 ** 3
            const total = snapshot.memory?.total / giga
            byId('settingsMemoryContentRight').hidden = !snapshot.memory
            if (snapshot.memory) {
                byId('settingsMemoryTotal').textContent = total.toFixed(1) + 'G'
                byId('settingsMemoryAvail').textContent = (snapshot.memory.available / giga).toFixed(1) + 'G'
            }
            const recommended = snapshot.recommendedMib / 1024
            const min = gibibytes(snapshot.options.minRAM, recommended)
            const max = gibibytes(snapshot.options.maxRAM, recommended)
            minimum = Math.min(snapshot.minimumMib / 1024, min, max)
            const hostLimit = Number.isFinite(total)
                ? Math.floor(total > 16 ? total - 4 - (total - 16) / 8 : total * 0.75)
                : 8
            upperLimit = Math.max(minimum + step, hostLimit, Math.ceil(min), Math.ceil(max))
            for (const { slider } of fields) {
                slider.setAttribute('aria-valuemin', minimum)
                slider.setAttribute('aria-valuemax', upperLimit)
            }
            setValue(0, Math.max(minimum, min))
            setValue(1, Math.max(minimum, max))
            rangeChanged = false
            originalArguments = (snapshot.options.jvmOptions ?? [])
                .map(option => /[\s"]/u.test(option) ? '"' + option.replace(/[\\"]/g, '\\$&') + '"' : option)
                .join(' ')
            argumentsInput.value = originalArguments
            originalOptions = snapshot.options
        },
        change() {
            const editedArguments = argumentsInput.value !== originalArguments
            if (!rangeChanged && !editedArguments) return null
            const options = { ...originalOptions }
            if (rangeChanged) {
                options.minRAM = Math.round(getValue(0) * 1024) + 'M'
                options.maxRAM = Math.round(getValue(1) * 1024) + 'M'
            }
            if (editedArguments) options.jvmOptions = parseOptions(argumentsInput.value)
            return { serverId: activeServerId, options }
        }
    }
}
