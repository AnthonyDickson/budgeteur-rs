function updateApplyButtonState() {
    const applyButton = document.getElementById('untagged-apply-button');
    if (!applyButton) {
        return;
    }

    const form = applyButton.closest('form');
    if (!form) {
        return;
    }

    const hasSelection = form.querySelector(
        'input[type="radio"][name^="tag_id_"]:checked, input[type="checkbox"][name="dismiss"]:checked'
    );
    applyButton.disabled = !hasSelection;
}

document.addEventListener('change', function (event) {
    const target = event.target;
    if (!(target instanceof HTMLInputElement)) {
        return;
    }

    const transactionId = target.getAttribute('data-transaction-id');
    if (!transactionId) {
        return;
    }

    if (target.type === 'radio' && target.name.startsWith('tag_id_') && target.checked) {
        const dismissCheckbox = document.querySelector(
            'input[type="checkbox"][name="dismiss"][data-transaction-id="' + transactionId + '"]'
        );
        if (dismissCheckbox) {
            dismissCheckbox.checked = false;
        }
        updateApplyButtonState();
        return;
    }

    if (target.type === 'checkbox' && target.name === 'dismiss' && target.checked) {
        const radios = document.querySelectorAll(
            'input[type="radio"][name="tag_id_' + transactionId + '"]'
        );
        radios.forEach(function (radio) {
            radio.checked = false;
        });
    }

    updateApplyButtonState();
});

document.addEventListener('DOMContentLoaded', updateApplyButtonState);
document.addEventListener('htmx:afterSwap', updateApplyButtonState);
