CREATE TABLE "ai_messages" (
	"id" varchar(60) PRIMARY KEY NOT NULL,
	"thread_id" varchar(50) NOT NULL,
	"user_id" varchar(40),
	"role" varchar(20) NOT NULL,
	"content" text NOT NULL,
	"metadata" jsonb,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "ai_threads" (
	"id" varchar(50) PRIMARY KEY NOT NULL,
	"user_id" varchar(40) NOT NULL,
	"title" varchar(200),
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "drafts" (
	"id" varchar(50) PRIMARY KEY NOT NULL,
	"user_id" varchar(40) NOT NULL,
	"project_id" varchar(40) NOT NULL,
	"step" integer NOT NULL,
	"payload_json" jsonb NOT NULL,
	"autosave_version" integer DEFAULT 0 NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL,
	CONSTRAINT "drafts_project_step_unique" UNIQUE("project_id","step")
);
--> statement-breakpoint
CREATE TABLE "profiles" (
	"user_id" varchar(40) PRIMARY KEY NOT NULL,
	"display_name" varchar(120),
	"bio" text,
	"company" varchar(120),
	"website" varchar(255),
	"avatar_url" text,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "projects" (
	"id" varchar(40) PRIMARY KEY NOT NULL,
	"user_id" varchar(40) NOT NULL,
	"name" varchar(160) NOT NULL,
	"slug" varchar(160) NOT NULL,
	"description" text,
	"status" varchar(30) DEFAULT 'draft' NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL,
	CONSTRAINT "projects_slug_unique" UNIQUE("slug")
);
--> statement-breakpoint
CREATE TABLE "users" (
	"id" varchar(40) PRIMARY KEY NOT NULL,
	"email" varchar(255) NOT NULL,
	"name" varchar(120),
	"image" text,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL,
	CONSTRAINT "users_email_unique" UNIQUE("email")
);
--> statement-breakpoint
ALTER TABLE "ai_messages" ADD CONSTRAINT "ai_messages_thread_id_ai_threads_id_fk" FOREIGN KEY ("thread_id") REFERENCES "public"."ai_threads"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "ai_messages" ADD CONSTRAINT "ai_messages_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "ai_threads" ADD CONSTRAINT "ai_threads_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "drafts" ADD CONSTRAINT "drafts_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "drafts" ADD CONSTRAINT "drafts_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "profiles" ADD CONSTRAINT "profiles_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "projects" ADD CONSTRAINT "projects_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "ai_messages_thread_idx" ON "ai_messages" USING btree ("thread_id");--> statement-breakpoint
CREATE INDEX "ai_messages_role_idx" ON "ai_messages" USING btree ("role");--> statement-breakpoint
CREATE INDEX "ai_threads_user_idx" ON "ai_threads" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "drafts_user_idx" ON "drafts" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "drafts_project_idx" ON "drafts" USING btree ("project_id");--> statement-breakpoint
CREATE INDEX "projects_user_idx" ON "projects" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "users_created_idx" ON "users" USING btree ("created_at");