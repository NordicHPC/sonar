from subprocess import check_output, SubprocessError, TimeoutExpired, DEVNULL


def safe_command(command: str, timeout_seconds: int) -> str:
    try:
        output = check_output(
            command, shell=True, stderr=DEVNULL, timeout=timeout_seconds
        ).decode("utf8")
        return output
    except TimeoutExpired:
        return ""
    except SubprocessError:
        return ""
