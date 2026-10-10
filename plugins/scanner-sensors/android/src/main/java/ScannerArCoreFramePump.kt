package com.xtreemze.scanner.sensors

import android.os.Looper
import android.view.Choreographer
import com.google.ar.core.Frame
import com.google.ar.core.Session

/**
 * Drives ARCore updates entirely on the native side.
 *
 * Scanner uses ARCore's hardware-buffer texture mode for this pump, so Session.update() does not
 * depend on a WebView/GL camera texture. The callback forwards only fresh Frames into the existing
 * JNI -> scanner-core ingestion path; it never owns SessionWorld or reconstruction state.
 */
class ScannerArCoreFramePump(
    private val session: Session,
    private val onFrame: (Frame) -> Unit,
    private val onFailure: (Exception) -> Unit,
) : Choreographer.FrameCallback {
    private var running = false
    private var lastFrameTimestampNanos = Long.MIN_VALUE

    val isRunning: Boolean
        get() = running

    fun start() {
        check(Looper.myLooper() == Looper.getMainLooper()) {
            "ARCore frame pump must be started on the Android main thread"
        }
        if (running) return
        running = true
        Choreographer.getInstance().postFrameCallback(this)
    }

    fun stop() {
        if (!running) return
        running = false
        Choreographer.getInstance().removeFrameCallback(this)
    }

    override fun doFrame(frameTimeNanos: Long) {
        if (!running) return

        try {
            val frame = session.update()
            val timestamp = frame.timestamp
            if (timestamp > 0L && timestamp != lastFrameTimestampNanos) {
                lastFrameTimestampNanos = timestamp
                onFrame(frame)
            }
        } catch (error: Exception) {
            running = false
            onFailure(error)
            return
        }

        if (running) {
            Choreographer.getInstance().postFrameCallback(this)
        }
    }
}
