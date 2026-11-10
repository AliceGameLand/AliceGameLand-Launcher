exports.bindUiClickSound = function(document, window){
    let uiAudioContext
    let uiClickBuffer
    let uiSoundAvailable = true
    let lastUiSoundAt = -Infinity

    /** A short, rounded tap with a soft low body instead of a noise click. */
    function createUiClickSamples(rate){
        const samples = new Float32Array(Math.round(rate * 0.04))
        for(let i = 0; i < samples.length; i++){
            const time = i / rate
            const attack = Math.min(1, time / 0.0005)
            const phase = 2 * Math.PI * (520 * time + 160 * 0.0028 * (1 - Math.exp(-time / 0.0028)))
            const body = Math.sin(phase) * Math.exp(-time / 0.0065) * 0.1
            const touch = Math.sin(2 * Math.PI * 1400 * time) * Math.exp(-time / 0.003) * 0.035
            const low = Math.sin(2 * Math.PI * 280 * time) * Math.exp(-time / 0.008) * 0.025
            samples[i] = (body + touch + low) * attack
        }
        return samples
    }

    /** Play a quiet click only for actual launcher button interactions. */
    function playUiButtonSound(){
        if(!uiSoundAvailable || !window.AudioContext) return

        try {
            if(!uiAudioContext || uiAudioContext.state === 'closed'){
                uiAudioContext = new window.AudioContext()
                uiClickBuffer = null
            }
            if(uiAudioContext.state === 'suspended'){
                void uiAudioContext.resume().catch(() => {
                    uiSoundAvailable = false
                })
            }

            if(!uiClickBuffer){
                const rate = uiAudioContext.sampleRate
                const samples = createUiClickSamples(rate)
                uiClickBuffer = uiAudioContext.createBuffer(1, samples.length, rate)
                uiClickBuffer.getChannelData(0).set(samples)
            }
            const tap = uiAudioContext.createBufferSource()
            const filter = uiAudioContext.createBiquadFilter()
            filter.type = 'lowpass'
            filter.frequency.value = 2400
            filter.Q.value = 0.5
            tap.buffer = uiClickBuffer
            tap.connect(filter)
            filter.connect(uiAudioContext.destination)
            tap.onended = () => {
                tap.disconnect()
                filter.disconnect()
            }
            tap.start()
        } catch {
            // Audio is optional; do not interrupt login or launching without an output device.
            uiSoundAvailable = false
        }
    }

    document.addEventListener('click', (event) => {
        if(!event.isTrusted || !(event.target instanceof window.Element)) return

        const control = event.target.closest('button, a[href], input[type="checkbox"]')
        if(!control || control.disabled || !control.closest('#main, #overlayContainer, #frameHomeActions')) return
        if(event.timeStamp - lastUiSoundAt < 80) return

        lastUiSoundAt = event.timeStamp
        playUiButtonSound()
    }, true)
}
