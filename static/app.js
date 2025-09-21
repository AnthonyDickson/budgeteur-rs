document.addEventListener('DOMContentLoaded', function () {
    // Find all buttons that toggle collapsible elements
    const toggleButtons = document.querySelectorAll('[data-collapse-toggle]');

    toggleButtons.forEach(function(toggleButton) {
        const targetId = toggleButton.getAttribute('data-collapse-toggle');
        const targetElement = document.getElementById(targetId);

        if (targetElement) {
            toggleButton.addEventListener('click', function () {
                // Toggle the 'hidden' class on the target element
                targetElement.classList.toggle('hidden');

                // Also toggle the aria-expanded attribute
                const isExpanded = toggleButton.getAttribute('aria-expanded') === 'true';
                toggleButton.setAttribute('aria-expanded', !isExpanded);
            });
        }
    });
});
