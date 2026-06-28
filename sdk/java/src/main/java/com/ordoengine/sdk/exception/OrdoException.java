package com.ordoengine.sdk.exception;

public class OrdoException extends RuntimeException {
    public OrdoException(String message) {
        super(message);
    }

    public OrdoException(String message, Throwable cause) {
        super(message, cause);
    }
}
