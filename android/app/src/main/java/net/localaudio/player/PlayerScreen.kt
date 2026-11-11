package net.localaudio.player

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.platform.LocalContext
import android.content.Intent
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.util.Locale
import kotlin.math.log10
import kotlin.math.roundToInt

data class Target(val name: String, val host: String, val port: Int)

@Composable
fun PlayerScreen(
    settings: Settings,
    servers: SnapshotStateList<FoundServer>,
    discoveryProblem: String?,
    autoConnect: Target?,
) {
    var stats by remember { mutableStateOf(PlayStats(Phase.Idle)) }
    var target by remember {
        mutableStateOf(
            autoConnect ?: settings.lastHost.takeIf { it.isNotBlank() }?.let {
                Target(settings.lastName.ifBlank { it }, it, settings.lastPort)
            },
        )
    }
    var buffer by remember { mutableIntStateOf(settings.bufferMs) }
    var volume by remember { mutableIntStateOf(settings.volume) }
    var passphrase by remember { mutableStateOf(settings.passphrase) }
    val context = LocalContext.current.applicationContext
    val client = remember { StreamClient(context) { stats = it } }

    val connect: (Target) -> Unit = { next ->
        target = next
        settings.lastHost = next.host
        settings.lastPort = next.port
        settings.lastName = next.name
        client.bufferMs = buffer
        client.volumePercent = volume
        client.start(next.host, next.port, passphrase)
    }

    LaunchedEffect(stats.remoteVolume) {
        stats.remoteVolume?.let {
            volume = it
            settings.volume = it
        }
    }
    LaunchedEffect(stats.active) {
        val service = Intent(context, PlaybackService::class.java)
        if (stats.active) context.startForegroundService(service) else context.stopService(service)
    }
    DisposableEffect(Unit) {
        autoConnect?.let(connect)
        onDispose {
            client.stop()
            context.stopService(Intent(context, PlaybackService::class.java))
        }
    }

    val current = target
    val shownName = current?.let { t -> servers.firstOrNull { it.host == t.host }?.name ?: t.name }

    Scaffold(containerColor = MaterialTheme.colorScheme.background) { insets ->
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(
                start = 20.dp,
                end = 20.dp,
                top = insets.calculateTopPadding() + 16.dp,
                bottom = insets.calculateBottomPadding() + 28.dp,
            ),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item { Header() }
            item {
                NowCard(
                    stats = stats,
                    name = shownName,
                    target = current,
                    onListen = { current?.let(connect) },
                    onStop = { client.stop() },
                )
            }
            item { SectionLabel("Computers on this network") }
            if (servers.isEmpty()) {
                item { ScanningCard(discoveryProblem) }
            }
            items(servers, key = { "${it.host}:${it.tcpPort}" }) { server ->
                ServerRow(
                    server = server,
                    live = stats.active && current?.host == server.host,
                    onClick = { connect(Target(server.name, server.host, server.tcpPort)) },
                )
            }
            item {
                ManualCard(
                    initialHost = current?.host.orEmpty(),
                    initialPort = current?.port ?: 47321,
                    onConnect = { host, port -> connect(Target(host, host, port)) },
                )
            }
            item { SectionLabel("Playback") }
            item {
                SettingsCard(
                    buffer = buffer,
                    onBuffer = {
                        buffer = it
                        settings.bufferMs = it
                        client.bufferMs = it
                    },
                    volume = volume,
                    onVolume = {
                        volume = it
                        settings.volume = it
                        client.volumePercent = it
                    },
                    passphrase = passphrase,
                    onPassphrase = {
                        passphrase = it
                        settings.passphrase = it
                    },
                )
            }
            item {
                Text(
                    "No ads · No account · Audio stays on your network",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 12.dp),
                )
            }
        }
    }
}

@Composable
private fun Header() {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 8.dp)) {
        AppMark(Modifier.size(44.dp))
        Spacer(Modifier.width(14.dp))
        Column {
            Text("LocalAudio", style = MaterialTheme.typography.headlineMedium)
            Text(
                "Your computer's sound, on this phone",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun AppMark(modifier: Modifier = Modifier) {
    val background = MaterialTheme.colorScheme.primaryContainer
    val bars = MaterialTheme.colorScheme.primary
    Canvas(
        modifier
            .clip(RoundedCornerShape(14.dp))
            .background(background),
    ) {
        val heights = listOf(0.34f, 0.62f, 0.46f, 0.24f)
        val gap = size.width / (heights.size + 1)
        val stroke = size.width * 0.11f
        heights.forEachIndexed { index, height ->
            val x = gap * (index + 1)
            val half = size.height * height / 2f
            drawLine(
                color = bars,
                start = Offset(x, size.height / 2f - half),
                end = Offset(x, size.height / 2f + half),
                strokeWidth = stroke,
                cap = StrokeCap.Round,
            )
        }
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(
        text.uppercase(Locale.ROOT),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 4.dp, top = 12.dp),
    )
}

@Composable
private fun NowCard(
    stats: PlayStats,
    name: String?,
    target: Target?,
    onListen: () -> Unit,
    onStop: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Surface(
        shape = MaterialTheme.shapes.extraLarge,
        color = colors.surfaceContainerHigh,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(22.dp)) {
            StatusPill(stats)
            Spacer(Modifier.height(16.dp))
            Text(
                text = when {
                    name != null -> name
                    else -> "Nothing playing"
                },
                style = MaterialTheme.typography.titleLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(4.dp))
            Text(
                text = when (stats.phase) {
                    Phase.Error -> stats.message
                    Phase.Playing -> "${target?.host.orEmpty()} · Opus, 48 kHz stereo"
                    Phase.Buffering -> stats.message.ifBlank { "Filling the buffer…" }
                    Phase.Connecting -> "Connecting to ${target?.host.orEmpty()}…"
                    Phase.Idle -> if (target != null) {
                        "Last used · ${target.host}"
                    } else {
                        "Pick a computer below to start listening."
                    }
                },
                style = MaterialTheme.typography.bodyMedium,
                color = if (stats.phase == Phase.Error) colors.error else colors.onSurfaceVariant,
            )
            Spacer(Modifier.height(22.dp))
            LevelMeter(level = if (stats.phase == Phase.Playing) levelOf(stats.rms) else 0f)
            if (stats.active) {
                Spacer(Modifier.height(18.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Stat("Received", compactCount(stats.packets))
                    Stat("Lost", lossText(stats))
                    Stat("Delay", if (stats.phase == Phase.Playing) "${stats.delayMs} ms" else "–")
                }
            }
            Spacer(Modifier.height(22.dp))
            when {
                stats.active -> FilledTonalButton(
                    onClick = onStop,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(52.dp),
                ) {
                    Icon(Icons.Filled.Close, contentDescription = null, modifier = Modifier.size(20.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Stop listening")
                }
                target != null -> Button(
                    onClick = onListen,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(52.dp),
                ) {
                    Icon(Icons.Filled.PlayArrow, contentDescription = null, modifier = Modifier.size(22.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(
                        if (stats.phase == Phase.Error) "Try again" else "Listen to ${name ?: target.host}",
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

@Composable
private fun StatusPill(stats: PlayStats) {
    val colors = MaterialTheme.colorScheme
    val (label, tint) = when (stats.phase) {
        Phase.Idle -> "Not connected" to colors.onSurfaceVariant
        Phase.Connecting -> "Connecting" to colors.tertiary
        Phase.Buffering -> "Buffering" to colors.tertiary
        Phase.Playing -> "Live" to colors.primary
        Phase.Error -> "Disconnected" to colors.error
    }
    val pulse = rememberInfiniteTransition(label = "pulse")
    val pulseAlpha by pulse.animateFloat(
        initialValue = 1f,
        targetValue = 0.35f,
        animationSpec = infiniteRepeatable(tween(900), RepeatMode.Reverse),
        label = "pulseAlpha",
    )
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .clip(CircleShape)
            .background(tint.copy(alpha = 0.14f))
            .padding(horizontal = 12.dp, vertical = 6.dp),
    ) {
        Box(
            Modifier
                .size(8.dp)
                .alpha(if (stats.phase == Phase.Playing) pulseAlpha else 1f)
                .clip(CircleShape)
                .background(tint),
        )
        Spacer(Modifier.width(8.dp))
        Text(label, style = MaterialTheme.typography.labelLarge, color = tint)
    }
}

@Composable
private fun LevelMeter(level: Float) {
    val animated by animateFloatAsState(level, animationSpec = tween(140), label = "level")
    val lit = MaterialTheme.colorScheme.primary
    val hot = MaterialTheme.colorScheme.tertiary
    val off = MaterialTheme.colorScheme.surfaceVariant
    Canvas(
        Modifier
            .fillMaxWidth()
            .height(10.dp),
    ) {
        val segments = 32
        val gap = 3.dp.toPx()
        val width = (size.width - gap * (segments - 1)) / segments
        val litCount = (animated * segments).roundToInt()
        for (index in 0 until segments) {
            val color = when {
                index >= litCount -> off
                index >= segments * 0.85f -> hot
                else -> lit
            }
            drawRoundRect(
                color = color,
                topLeft = Offset(index * (width + gap), 0f),
                size = Size(width, size.height),
                cornerRadius = CornerRadius(3.dp.toPx()),
            )
        }
    }
}

@Composable
private fun Stat(label: String, value: String) {
    Column {
        Text(
            label.uppercase(Locale.ROOT),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(2.dp))
        Text(value, style = MaterialTheme.typography.titleMedium)
    }
}

@Composable
private fun ScanningCard(problem: String?) {
    Surface(
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(Modifier.padding(18.dp), verticalAlignment = Alignment.CenterVertically) {
            if (problem == null) {
                CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp)
            } else {
                Icon(Icons.Filled.Warning, contentDescription = null, tint = MaterialTheme.colorScheme.tertiary)
            }
            Spacer(Modifier.width(16.dp))
            Column {
                Text(
                    if (problem == null) "Looking for computers…" else "Can't search this network",
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    problem ?: "Open LocalAudio on your computer. Both need to be on the same Wi-Fi.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun ServerRow(server: FoundServer, live: Boolean, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Surface(
        onClick = onClick,
        shape = MaterialTheme.shapes.large,
        color = colors.surfaceContainer,
        border = if (live) BorderStroke(1.5.dp, colors.primary) else null,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .size(44.dp)
                    .clip(CircleShape)
                    .background(colors.primaryContainer),
            ) {
                Text(
                    server.name.take(1).uppercase(Locale.ROOT),
                    style = MaterialTheme.typography.titleMedium,
                    color = colors.onPrimaryContainer,
                )
            }
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Text(server.name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(server.host, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
            }
            if (live) {
                Text("Playing", style = MaterialTheme.typography.labelLarge, color = colors.primary)
            } else {
                Box(
                    contentAlignment = Alignment.Center,
                    modifier = Modifier
                        .size(36.dp)
                        .clip(CircleShape)
                        .background(colors.primary.copy(alpha = 0.14f)),
                ) {
                    Icon(Icons.Filled.PlayArrow, contentDescription = "Listen", tint = colors.primary)
                }
            }
        }
    }
}

@Composable
private fun ManualCard(initialHost: String, initialPort: Int, onConnect: (String, Int) -> Unit) {
    var expanded by rememberSaveable { mutableStateOf(false) }
    var host by rememberSaveable(initialHost) { mutableStateOf(initialHost) }
    var port by rememberSaveable(initialPort) { mutableStateOf(initialPort.toString()) }
    val arrow by animateFloatAsState(if (expanded) 180f else 0f, label = "arrow")
    Surface(
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { expanded = !expanded }
                    .padding(18.dp),
            ) {
                Column(Modifier.weight(1f)) {
                    Text("Connect by address", style = MaterialTheme.typography.titleSmall)
                    Text(
                        "For USB tethering, or if your computer isn't listed",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Icon(Icons.Filled.KeyboardArrowDown, contentDescription = null, modifier = Modifier.rotate(arrow))
            }
            AnimatedVisibility(expanded) {
                Column(
                    Modifier.padding(start = 18.dp, end = 18.dp, bottom = 18.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        OutlinedTextField(
                            value = host,
                            onValueChange = { host = it.trim() },
                            label = { Text("Address") },
                            placeholder = { Text("192.168.0.10") },
                            singleLine = true,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                            modifier = Modifier.weight(1f),
                        )
                        OutlinedTextField(
                            value = port,
                            onValueChange = { port = it.filter(Char::isDigit).take(5) },
                            label = { Text("Port") },
                            singleLine = true,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                            modifier = Modifier.width(96.dp),
                        )
                    }
                    Button(
                        onClick = { onConnect(host, port.toIntOrNull() ?: 47321) },
                        enabled = host.isNotBlank(),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(48.dp),
                    ) {
                        Text("Connect")
                    }
                }
            }
        }
    }
}

@Composable
private fun SettingsCard(
    buffer: Int,
    onBuffer: (Int) -> Unit,
    volume: Int,
    onVolume: (Int) -> Unit,
    passphrase: String,
    onPassphrase: (String) -> Unit,
) {
    var showPassphrase by remember { mutableStateOf(false) }
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Surface(
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(18.dp)) {
            SliderHeader("Buffer", "$buffer ms")
            Slider(
                value = buffer.toFloat(),
                onValueChange = { onBuffer(((it / 10f).roundToInt() * 10).coerceIn(20, 200)) },
                valueRange = 20f..200f,
            )
            Row(Modifier.fillMaxWidth()) {
                Text("Less delay", style = MaterialTheme.typography.bodySmall, color = muted)
                Spacer(Modifier.weight(1f))
                Text("Fewer dropouts", style = MaterialTheme.typography.bodySmall, color = muted)
            }
            HorizontalDivider(Modifier.padding(vertical = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
            SliderHeader("Volume", "$volume%")
            Slider(
                value = volume.toFloat(),
                onValueChange = { onVolume(it.roundToInt()) },
                valueRange = 0f..100f,
            )
            HorizontalDivider(Modifier.padding(vertical = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
            OutlinedTextField(
                value = passphrase,
                onValueChange = onPassphrase,
                label = { Text("Passphrase") },
                placeholder = { Text("Leave empty if none") },
                supportingText = { Text("Only needed if your computer has one set.") },
                singleLine = true,
                visualTransformation = if (showPassphrase) VisualTransformation.None else PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                trailingIcon = {
                    if (passphrase.isNotEmpty()) {
                        TextButton(onClick = { showPassphrase = !showPassphrase }) {
                            Text(if (showPassphrase) "Hide" else "Show")
                        }
                    }
                },
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

@Composable
private fun SliderHeader(title: String, value: String) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = MaterialTheme.typography.titleSmall)
        Spacer(Modifier.weight(1f))
        Text(value, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
    }
}

private fun levelOf(rms: Int): Float {
    if (rms <= 0) return 0f
    val db = 20.0 * log10(rms / 32768.0)
    return ((db + 60.0) / 60.0).coerceIn(0.0, 1.0).toFloat()
}

private fun compactCount(value: Long): String = when {
    value >= 1_000_000 -> String.format(Locale.ROOT, "%.1fM", value / 1_000_000.0)
    value >= 1_000 -> String.format(Locale.ROOT, "%.1fk", value / 1_000.0)
    else -> value.toString()
}

private fun lossText(stats: PlayStats): String {
    val total = stats.packets + stats.lost
    if (total == 0L) return "0%"
    return String.format(Locale.ROOT, "%.1f%%", stats.lost * 100.0 / total)
}
