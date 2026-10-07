package net.havenkeys.android.ui.health

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.Pill
import net.havenkeys.android.ui.kit.PillTone
import uniffi.havenkeys_mobile.HealthKind

/** The check's name, as its row on the Health screen shows it. */
@StringRes
internal fun HealthKind.titleRes(): Int = when (this) {
    HealthKind.REUSED -> R.string.health_reused_title
    HealthKind.WEAK -> R.string.health_weak_title
    HealthKind.INSECURE -> R.string.health_insecure_title
    HealthKind.DUPLICATE -> R.string.health_duplicate_title
    HealthKind.PASSKEY -> R.string.health_passkey_title
    HealthKind.TWO_FACTOR -> R.string.health_two_factor_title
    HealthKind.OLD -> R.string.health_old_title
}

/** Why the check matters, in one or two sentences. */
@StringRes
internal fun HealthKind.bodyRes(): Int = when (this) {
    HealthKind.REUSED -> R.string.health_reused_body
    HealthKind.WEAK -> R.string.health_weak_body
    HealthKind.INSECURE -> R.string.health_insecure_body
    HealthKind.DUPLICATE -> R.string.health_duplicate_body
    HealthKind.PASSKEY -> R.string.health_passkey_body
    HealthKind.TWO_FACTOR -> R.string.health_two_factor_body
    HealthKind.OLD -> R.string.health_old_body
}

/** Checks a new password fixes: their rows offer "Change password". */
internal val PASSWORD_KINDS = setOf(HealthKind.WEAK, HealthKind.REUSED, HealthKind.OLD)

/** Checks a site setting fixes: their rows offer "How to enable" when Rust has a link. */
internal val HELP_KINDS = setOf(HealthKind.PASSKEY, HealthKind.TWO_FACTOR)

/** One check as a chip says it about this login; [groupSize] and [duplicates] are already clamped. */
@Composable
internal fun chipLabel(kind: HealthKind, groupSize: Int?, duplicates: Int?): String = when (kind) {
    HealthKind.WEAK -> stringResource(R.string.health_chip_weak)
    HealthKind.REUSED -> {
        val n = groupSize ?: reusedIn(0)
        pluralStringResource(R.plurals.health_chip_reused, n, n)
    }
    HealthKind.OLD -> stringResource(R.string.health_chip_old)
    HealthKind.PASSKEY -> stringResource(R.string.health_chip_passkey)
    HealthKind.TWO_FACTOR -> stringResource(R.string.health_chip_two_factor)
    HealthKind.INSECURE -> stringResource(R.string.health_chip_insecure)
    HealthKind.DUPLICATE -> {
        val n = duplicates ?: duplicatesOf(0)
        pluralStringResource(R.plurals.health_chip_duplicate, n, n)
    }
}

/**
 * The checks a login fails, as outlined markers in the screen's order. Not
 * controls: the row or the screen around them acts. Our words wrap, never cut.
 */
@Composable
fun HealthChips(kinds: List<HealthKind>, groupSize: Int?, duplicates: Int?, modifier: Modifier = Modifier) {
    if (kinds.isEmpty()) return
    FlowRow(
        modifier,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        KIND_ORDER.filter { it in kinds }.forEach { kind ->
            Pill(chipLabel(kind, groupSize, duplicates), tone = PillTone.Outline)
        }
    }
}
