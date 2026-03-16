package com.ordoengine.sdk.exception;

public class ApiException extends OrdoException {
    private final String errorCode;
    private final int statusCode;

    public ApiException(String message, String errorCode, int statusCode) {
        super(message);
        this.errorCode = errorCode;
        this.statusCode = statusCode;
    }

    public ApiException(String message, String errorCode, int statusCode, Throwable cause) {
        super(message, cause);
        this.errorCode = errorCode;
        this.statusCode = statusCode;
    }

    public String getErrorCode() {
        return errorCode;
    }

    public int getStatusCode() {
        return statusCode;
    }

    public boolean isRetryable() {
        return statusCode >= 500 || statusCode == 429;
    }
}
