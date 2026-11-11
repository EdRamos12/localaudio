package net.localaudio.player

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf

class MainActivity : ComponentActivity() {
    private var discovery: Discovery? = null
    private val servers = mutableStateListOf<FoundServer>()
    private val discoveryProblem = mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        discovery = Discovery(
            this,
            onServer = { server ->
                runOnUiThread {
                    if (servers.none { it.host == server.host && it.tcpPort == server.tcpPort }) {
                        servers.add(server)
                    }
                }
            },
            onLog = { message ->
                if (message.startsWith("Discovery failed")) {
                    runOnUiThread { discoveryProblem.value = message }
                }
            },
        ).also { it.start() }

        val settings = Settings(this)
        val presetHost = intent.getStringExtra("host").orEmpty()
        val presetPort = intent.getIntExtra("port", 47321)
        setContent {
            LocalAudioTheme {
                PlayerScreen(
                    settings = settings,
                    servers = servers,
                    discoveryProblem = discoveryProblem.value,
                    autoConnect = presetHost.takeIf { it.isNotBlank() }?.let { Target(it, it, presetPort) },
                )
            }
        }
    }

    override fun onDestroy() {
        discovery?.stop()
        super.onDestroy()
    }
}
